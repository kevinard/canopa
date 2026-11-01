//! Bounded, half-duplex Assuan pipe server. stdout belongs exclusively to the peer.
use canopa_session_ui::secret::Secret;
use std::io::{self, BufRead, Read, Write};
use zeroize::Zeroizing;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pin,
    Confirm,
    Message,
}

#[derive(Clone)]
pub struct Request {
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub prompt: String,
    pub ok: String,
    pub cancel: String,
    pub notok: String,
    pub error: String,
    pub repeat: Option<String>,
    pub repeat_error: String,
    pub timeout: u64,
    pub focus_cancel: bool,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            kind: Kind::Pin,
            title: String::new(),
            description: String::new(),
            prompt: "Passphrase:".into(),
            ok: "OK".into(),
            cancel: "Cancel".into(),
            notok: String::new(),
            error: String::new(),
            repeat: None,
            repeat_error: "Passphrases do not match. Try again.".into(),
            timeout: 120,
            focus_cancel: false,
        }
    }
}
pub enum Response {
    Pin { secret: Secret, repeated: bool },
    Accepted,
    Canceled,
    NotConfirmed,
    Timeout,
    Unavailable,
}

fn error(writer: &mut impl Write, code: u32, message: &str) -> io::Result<()> {
    // GPG_ERR_SOURCE_PINENTRY (5), with libgpg-error's published numeric codes.
    writeln!(
        writer,
        "ERR {} {message} <Canopa Pinentry>",
        (5 << 24) | code
    )
}
fn unescape(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut source = value.as_bytes().iter().copied();
    while let Some(byte) = source.next() {
        let byte = if byte == b'%' {
            let high = (source.next()? as char).to_digit(16)?;
            let low = (source.next()? as char).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        };
        if byte == 0 {
            return None;
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).ok()
}
/// Escape into erased fixed storage, splitting data at Assuan's 1000-byte line limit.
fn data(writer: &mut impl Write, value: &[u8]) -> io::Result<()> {
    let mut frame = Zeroizing::new([0u8; 1000]);
    frame[..2].copy_from_slice(b"D ");
    let mut end = 2;
    for byte in value {
        let escape = matches!(byte, b'%' | b'\n' | b'\r');
        let needed = if escape { 3 } else { 1 };
        if end + needed > 999 {
            frame[end] = b'\n';
            writer.write_all(&frame[..=end])?;
            end = 2;
        }
        if escape {
            const HEX: &[u8] = b"0123456789ABCDEF";
            frame[end..end + 3].copy_from_slice(&[
                b'%',
                HEX[(byte >> 4) as usize],
                HEX[(byte & 15) as usize],
            ]);
        } else {
            frame[end] = *byte;
        }
        end += needed;
    }
    if end > 2 {
        frame[end] = b'\n';
        writer.write_all(&frame[..=end])?;
    }
    Ok(())
}

pub fn serve(
    mut reader: impl BufRead,
    mut writer: impl Write,
    timeout: u64,
    mut show: impl FnMut(&Request) -> Response,
) -> io::Result<()> {
    let mut defaults = Request {
        timeout,
        ..Request::default()
    };
    let mut request = defaults.clone();
    writeln!(writer, "OK Canopa Pinentry ready")?;
    writer.flush()?;
    loop {
        let mut line = Zeroizing::new(Vec::new());
        let count = reader.by_ref().take(1001).read_until(b'\n', &mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count > 1000 {
            error(&mut writer, 263, "Line too long")?;
            writer.flush()?;
            return Ok(());
        }
        if line.last() != Some(&b'\n') {
            return Ok(());
        }
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let Ok(line) = std::str::from_utf8(&line) else {
            error(&mut writer, 276, "Invalid encoding")?;
            writer.flush()?;
            continue;
        };
        let line = line.trim_start_matches([' ', '\t']);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (command, raw) = line.split_once([' ', '\t']).unwrap_or((line, ""));
        let command = command.to_ascii_uppercase();
        let Some(value) = unescape(raw.trim_start_matches([' ', '\t'])) else {
            error(&mut writer, 276, "Invalid argument encoding")?;
            writer.flush()?;
            continue;
        };
        let mut failure = None;
        match command.as_str() {
            "BYE" => {
                writeln!(writer, "OK closing connection")?;
                writer.flush()?;
                return Ok(());
            }
            "NOP" => {}
            "RESET" => request = defaults.clone(),
            "SETDESC" => request.description = value,
            "SETTITLE" => request.title = value,
            "SETPROMPT" => request.prompt = value,
            "SETOK" => request.ok = value,
            "SETCANCEL" => request.cancel = value,
            "SETNOTOK" => request.notok = value,
            "SETERROR" => request.error = value,
            "SETREPEAT" => request.repeat = Some(value),
            "SETREPEATERROR" => request.repeat_error = value,
            // Optional presentation hints; no external cache or quality inquiries.
            "SETREPEATOK" | "SETQUALITYBAR" | "SETQUALITYBAR_TT" | "SETKEYINFO" => {}
            "SETTIMEOUT" => match value.parse::<u32>() {
                Ok(seconds) => {
                    defaults.timeout = u64::from(seconds);
                    request.timeout = defaults.timeout;
                }
                Err(_) => failure = Some((55, "Invalid timeout")),
            },
            "OPTION" => {
                let (key, value) = value.split_once('=').unwrap_or((&value, ""));
                match key.trim_start_matches("--") {
                    "default-ok" => {
                        defaults.ok = value.into();
                        request.ok = value.into();
                    }
                    "default-cancel" => {
                        defaults.cancel = value.into();
                        request.cancel = value.into();
                    }
                    "default-prompt" => {
                        defaults.prompt = value.into();
                        request.prompt = value.into();
                    }
                    // Terminal/X11 metadata is accepted for gpg-agent interoperability.
                    // It never selects another display or causes terminal/file access.
                    "ttyname" | "ttytype" | "lc-ctype" | "lc-messages" | "display" => {}
                    _ => failure = Some((174, "Unknown option")),
                }
            }
            "GETINFO" => match value.as_str() {
                "pid" => data(&mut writer, std::process::id().to_string().as_bytes())?,
                "version" => data(&mut writer, env!("CARGO_PKG_VERSION").as_bytes())?,
                "flavor" => data(&mut writer, b"canopa")?,
                _ => failure = Some((55, "Unknown information")),
            },
            "GETPIN" | "CONFIRM" | "MESSAGE" => {
                let options: Vec<_> = value.split_whitespace().collect();
                if options
                    .iter()
                    .any(|v| !["--one-button", "--focus=ok", "--focus=cancel"].contains(v))
                    || (command == "GETPIN" && !options.is_empty())
                {
                    failure = Some((276, "Invalid dialog option"));
                } else {
                    request.kind = if command == "GETPIN" {
                        Kind::Pin
                    } else if command == "MESSAGE" || options.contains(&"--one-button") {
                        Kind::Message
                    } else {
                        Kind::Confirm
                    };
                    request.focus_cancel = options.contains(&"--focus=cancel");
                    match show(&request) {
                        Response::Pin { secret, repeated } if request.kind == Kind::Pin => {
                            if repeated {
                                writeln!(writer, "S PIN_REPEATED")?;
                            }
                            data(&mut writer, secret.as_bytes())?;
                        }
                        Response::Accepted if request.kind != Kind::Pin => {}
                        Response::NotConfirmed => failure = Some((114, "Not confirmed")),
                        Response::Timeout => failure = Some((62, "Timeout")),
                        Response::Unavailable => failure = Some((85, "No pinentry available")),
                        _ => failure = Some((99, "Operation canceled")),
                    }
                    request.error.clear();
                    if request.kind == Kind::Pin {
                        request.repeat = None;
                    }
                }
            }
            _ => failure = Some((275, "Unknown command")),
        }
        if let Some((code, message)) = failure {
            error(&mut writer, code, message)?;
        } else {
            writeln!(writer, "OK")?;
        }
        writer.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_escape_reset_defaults_and_repeat_are_serialized() {
        let mut output = Vec::new();
        let mut calls = 0;
        serve(&b"OPTION default-ok=_Yes\nSETDESC Key%0A100%25\nSETREPEAT Again\nGETPIN\nGETPIN\nRESET\nCONFIRM --focus=cancel\nBYE\n"[..], &mut output, 30, |r| {
            calls += 1;
            if calls < 3 {
                assert_eq!(r.description, "Key\n100%"); assert_eq!(r.repeat.is_some(), calls == 1);
                let mut secret = Secret::default(); secret.push("a%b");
                Response::Pin { secret, repeated: calls == 1 }
            } else { assert!(r.focus_cancel); assert!(r.description.is_empty()); assert_eq!(r.ok, "_Yes"); Response::NotConfirmed }
        }).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(calls, 3);
        assert_eq!(output.matches("S PIN_REPEATED").count(), 1);
        assert_eq!(output.matches("D a%25b").count(), 2);
        assert!(output.contains("ERR 83886194"));
    }
    #[test]
    fn malformed_unimplemented_and_oversized_requests_cannot_open_dialogs() {
        for input in [
            b"OPTION constraints-enforce\nSETDESC %00\nSETDESC %XX\nFOO\nBYE\n".to_vec(),
            vec![b'x'; 2000],
        ] {
            let mut output = Vec::new();
            serve(input.as_slice(), &mut output, 120, |_| {
                panic!("unexpected dialog")
            })
            .unwrap();
            assert!(String::from_utf8(output).unwrap().contains("ERR "));
        }
    }
    #[test]
    fn data_frames_round_trip_large_unicode_and_escaped_values() {
        let input = "🔒%\r\n".repeat(300);
        let mut output = Vec::new();
        data(&mut output, input.as_bytes()).unwrap();
        let mut decoded = Vec::new();
        for frame in output.split_inclusive(|b| *b == b'\n') {
            assert!(frame.len() <= 1000);
            let bytes = &frame[2..frame.len() - 1];
            let mut index = 0;
            while index < bytes.len() {
                if bytes[index] == b'%' {
                    decoded.push(
                        u8::from_str_radix(
                            std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap(),
                            16,
                        )
                        .unwrap(),
                    );
                    index += 3;
                } else {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
        }
        assert_eq!(decoded, input.as_bytes());
    }
}
