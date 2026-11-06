//! qpdf's public C object API supplies PDF objects. Rust owns sheet selection,
//! affine placement and the new page tree. This runs only in the namespace.
//! ABI: qpdf/qpdf-c.h (qpdf12); no private C++ objects are dereferenced.
use super::{
    Mode, Request,
    plan::{MAX_PAGES, Page, Plan},
};
use libloading::Library;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    fs::File,
    os::unix::fs::{FileExt, MetadataExt},
};
type Data = *mut c_void;
type Object = u32;

macro_rules! api {
    ($($field:ident($($arg:ty),*) -> $ret:ty = $symbol:literal;)*) => {
        struct Api { _library:Library, $($field:unsafe extern "C" fn($($arg),*) -> $ret,)* }
        impl Api {
            fn load() -> anyhow::Result<Self> {
                for name in ["libqpdf.so.30","libqpdf.so.29","libqpdf.so.28","libqpdf.so"] {
                    // SAFETY: only the trusted system PDF library is loaded,
                    // inside the disposable converter namespace.
                    if let Ok(library)=unsafe{Library::new(name)} {
                        // SAFETY: signatures below follow the public C header;
                        // Library is retained as long as any function is used.
                        return unsafe { Ok(Self { $($field:*library.get(concat!($symbol,"\0").as_bytes())?,)* _library:library }) };
                    }
                }
                anyhow::bail!("The qpdf runtime is unavailable")
            }
        }
    }
}
api! {
    init()->Data="qpdf_init";
    cleanup(*mut Data)->()="qpdf_cleanup";
    silence(Data)->()="qpdf_silence_errors";
    warnings(Data,c_int)->()="qpdf_set_suppress_warnings";
    error(Data)->c_int="qpdf_has_error";
    read(Data,*const c_char,*const c_char,u64,*const c_char)->c_int="qpdf_read_memory";
    inherited(Data)->c_int="qpdf_push_inherited_attributes_to_page";
    count(Data)->c_int="qpdf_get_num_pages";
    page(Data,usize)->Object="qpdf_get_page_n";
    trailer(Data)->Object="qpdf_get_trailer";
    update_pages(Data)->c_int="qpdf_update_all_pages_cache";
    dict(Data)->Object="qpdf_oh_new_dictionary";
    array(Data)->Object="qpdf_oh_new_array";
    null(Data)->Object="qpdf_oh_new_null";
    name(Data,*const c_char)->Object="qpdf_oh_new_name";
    integer(Data,i64)->Object="qpdf_oh_new_integer";
    parse(Data,*const c_char)->Object="qpdf_oh_parse";
    get(Data,Object,*const c_char)->Object="qpdf_oh_get_key";
    set(Data,Object,*const c_char,Object)->()="qpdf_oh_replace_key";
    dictionary(Data,Object)->c_int="qpdf_oh_is_dictionary";
    is_null(Data,Object)->c_int="qpdf_oh_is_null";
    is_number(Data,Object)->c_int="qpdf_oh_is_number";
    number(Data,Object)->f64="qpdf_oh_get_numeric_value";
    length(Data,Object)->c_int="qpdf_oh_get_array_n_items";
    item(Data,Object,c_int)->Object="qpdf_oh_get_array_item";
    append(Data,Object,Object)->()="qpdf_oh_append_item";
    indirect(Data,Object)->Object="qpdf_make_indirect_object";
    new_stream(Data)->Object="qpdf_oh_new_stream";
    stream_dict(Data,Object)->Object="qpdf_oh_get_dict";
    content(Data,Object,*mut *mut u8,*mut usize)->c_int="qpdf_oh_get_page_content_data";
    free(*mut *mut u8)->()="qpdf_oh_free_buffer";
    data(Data,Object,*const u8,usize,Object,Object)->()="qpdf_oh_replace_stream_data";
    begin_write(Data,*const c_char)->c_int="qpdf_init_write";
    preserve(Data,c_int)->()="qpdf_set_preserve_unreferenced_objects";
    write(Data)->c_int="qpdf_write";
}
struct Pdf {
    api: Api,
    data: Data,
    // qpdf_read_memory borrows this allocation until cleanup in Drop.
    _bytes: Vec<u8>,
}
impl Drop for Pdf {
    fn drop(&mut self) {
        // SAFETY: init returned this uniquely owned object; the library still lives.
        unsafe {
            (self.api.cleanup)(&mut self.data);
        }
    }
}
struct Buffer<'a> {
    api: &'a Api,
    pointer: *mut u8,
}
impl Drop for Buffer<'_> {
    fn drop(&mut self) {
        // SAFETY: qpdf allocated this buffer and frees/nulls it through this API.
        unsafe {
            (self.api.free)(&mut self.pointer);
        }
    }
}
impl Pdf {
    fn open(source: &File) -> anyhow::Result<Self> {
        let length = source.metadata()?.len();
        anyhow::ensure!(
            length > 0
                && length <= crate::print::worker::MAX_DOCUMENT_BYTES
                && rustix::fs::fcntl_get_seals(source)?.contains(
                    rustix::fs::SealFlags::WRITE
                        | rustix::fs::SealFlags::GROW
                        | rustix::fs::SealFlags::SHRINK
                        | rustix::fs::SealFlags::SEAL
                ),
            "Invalid frozen PDF source"
        );
        let mut bytes = vec![0; length as usize];
        source.read_exact_at(&mut bytes, 0)?;
        let api = Api::load()?;
        // SAFETY: qpdf_init returns a unique owner or a null allocation failure.
        let data = unsafe { (api.init)() };
        anyhow::ensure!(!data.is_null(), "PDF runtime allocation failed");
        let pdf = Self {
            api,
            data,
            _bytes: bytes,
        };
        // SAFETY: the input allocation is retained until after cleanup. All
        // handles stay on this converter thread; no password is supplied.
        unsafe {
            (pdf.api.silence)(data);
            (pdf.api.warnings)(data, 1);
            pdf.status((pdf.api.read)(
                data,
                c"print document".as_ptr(),
                pdf._bytes.as_ptr().cast(),
                length,
                std::ptr::null(),
            ))?;
            pdf.status((pdf.api.inherited)(data))?;
        }
        Ok(pdf)
    }
    fn pages(&self) -> anyhow::Result<Vec<(Object, Page)>> {
        // SAFETY: this Pdf has successfully loaded a frozen source.
        let count = unsafe { (self.api.count)(self.data) };
        anyhow::ensure!(
            count > 0 && count as usize <= MAX_PAGES,
            "Print page count exceeds limits"
        );
        (0..count as usize).map(|index| self.page(index)).collect()
    }
    fn check(&self) -> anyhow::Result<()> {
        // SAFETY: data and all its handles remain owned by this Pdf instance.
        anyhow::ensure!(
            unsafe { (self.api.error)(self.data) } == 0,
            "PDF object operation failed"
        );
        Ok(())
    }
    fn status(&self, status: c_int) -> anyhow::Result<()> {
        anyhow::ensure!(status & 2 == 0, "PDF processing failed");
        self.check()
    }
    fn get(&self, object: Object, key: &CStr) -> Object {
        // SAFETY: handles and keys are used only with their owning Pdf.
        unsafe { (self.api.get)(self.data, object, key.as_ptr()) }
    }
    fn set(&self, object: Object, key: &CStr, value: Object) {
        // SAFETY: all handles refer to this Pdf and the key lives through the call.
        unsafe { (self.api.set)(self.data, object, key.as_ptr(), value) }
    }
    fn parse(&self, text: String) -> anyhow::Result<Object> {
        let text = CString::new(text)?;
        // SAFETY: generated numeric PDF syntax is NUL terminated and lives here.
        let object = unsafe { (self.api.parse)(self.data, text.as_ptr()) };
        self.check()?;
        Ok(object)
    }
    fn name(&self, name: &CStr) -> Object {
        // SAFETY: name is a live canonical PDF name.
        unsafe { (self.api.name)(self.data, name.as_ptr()) }
    }
    fn number(&self, object: Object, default: Option<f64>) -> anyhow::Result<f64> {
        // SAFETY: object belongs to the retained Pdf.
        let result = unsafe {
            if (self.api.is_null)(self.data, object) != 0 {
                default.ok_or_else(|| anyhow::anyhow!("Missing PDF number"))?
            } else {
                anyhow::ensure!(
                    (self.api.is_number)(self.data, object) != 0,
                    "Invalid PDF number"
                );
                (self.api.number)(self.data, object)
            }
        };
        self.check()?;
        anyhow::ensure!(result.is_finite(), "Nonfinite PDF number");
        Ok(result)
    }
    fn stream(&self, bytes: &[u8]) -> anyhow::Result<Object> {
        // SAFETY: qpdf copies bytes before returning; handles share this owner.
        let object = unsafe {
            let object = (self.api.new_stream)(self.data);
            let null = (self.api.null)(self.data);
            (self.api.data)(self.data, object, bytes.as_ptr(), bytes.len(), null, null);
            object
        };
        self.check()?;
        Ok(object)
    }
    fn page(&self, index: usize) -> anyhow::Result<(Object, Page)> {
        // SAFETY: caller bounded index against qpdf's retained page count.
        let page = unsafe { (self.api.page)(self.data, index) };
        let mut bounds = self.get(page, c"/CropBox");
        // SAFETY: object handles are valid for this Pdf.
        if unsafe { (self.api.is_null)(self.data, bounds) } != 0 {
            bounds = self.get(page, c"/MediaBox");
        }
        // SAFETY: qpdf reports a type error for a non-array; check catches it.
        anyhow::ensure!(
            unsafe { (self.api.length)(self.data, bounds) } == 4,
            "Invalid PDF page bounds"
        );
        let mut rectangle = [0.; 4];
        for (index, value) in rectangle.iter_mut().enumerate() {
            // SAFETY: bounds is a four-element array and index is in range.
            *value = self.number(
                unsafe { (self.api.item)(self.data, bounds, index as c_int) },
                None,
            )?;
        }
        let rotation = self.number(self.get(page, c"/Rotate"), Some(0.))?;
        anyhow::ensure!(
            rotation.fract() == 0. && rotation.abs() <= 360_000.,
            "Invalid PDF rotation"
        );
        let shape = Page {
            bounds: rectangle,
            rotate: rotation as i32,
            unit: self.number(self.get(page, c"/UserUnit"), Some(1.))?,
        };
        shape.normalized()?;
        self.check()?;
        Ok((page, shape))
    }
    fn form(&self, page: Object, shape: Page, total: &mut usize) -> anyhow::Result<Object> {
        let mut pointer = std::ptr::null_mut();
        let mut length = 0;
        // SAFETY: output pointers are live locals; the returned allocation is
        // immediately guarded and released with qpdf's matching free function.
        let status = unsafe { (self.api.content)(self.data, page, &mut pointer, &mut length) };
        let buffer = Buffer {
            api: &self.api,
            pointer,
        };
        self.status(status)?;
        *total = total
            .checked_add(length)
            .ok_or_else(|| anyhow::anyhow!("PDF contents overflow"))?;
        anyhow::ensure!(
            *total <= crate::print::worker::MAX_DOCUMENT_BYTES as usize
                && (length == 0 || !buffer.pointer.is_null()),
            "PDF decoded contents exceed limits"
        );
        // SAFETY: successful qpdf call allocated at least length bytes; the
        // guard outlives the copy performed by stream(). Empty pages are valid.
        let bytes = if length == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(buffer.pointer, length) }
        };
        let form = self.stream(bytes)?;
        // SAFETY: form is a stream in this document.
        let dict = unsafe { (self.api.stream_dict)(self.data, form) };
        self.set(dict, c"/Type", self.name(c"/XObject"));
        self.set(dict, c"/Subtype", self.name(c"/Form"));
        self.set(dict, c"/BBox", self.parse(array(&shape.bounds))?);
        self.set(dict, c"/Resources", self.get(page, c"/Resources"));
        let group = self.get(page, c"/Group");
        // SAFETY: group belongs to this Pdf; only transparency dictionaries copy.
        if unsafe { (self.api.dictionary)(self.data, group) } != 0 {
            self.set(dict, c"/Group", group);
        }
        self.check()?;
        Ok(form)
    }
}
fn array(values: &[f64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| format!("{v:.12}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub(super) fn render(request: &Request) -> anyhow::Result<()> {
    let source = File::open("/proc/self/fd/0")?;
    let output = File::open("/proc/self/fd/1")?;
    let input = source.metadata()?;
    let target = output.metadata()?;
    anyhow::ensure!(
        input.is_file()
            && input.len() > 0
            && input.len() <= crate::print::worker::MAX_DOCUMENT_BYTES
            && rustix::fs::fcntl_get_seals(&source)?.contains(
                rustix::fs::SealFlags::WRITE
                    | rustix::fs::SealFlags::GROW
                    | rustix::fs::SealFlags::SHRINK
                    | rustix::fs::SealFlags::SEAL
            )
            && target.is_file()
            && target.len() == 0
            && (input.dev(), input.ino()) != (target.dev(), target.ino()),
        "Invalid PDF converter streams"
    );
    let pdf = Pdf::open(&source)?;
    let data = pdf.data;
    let pages = pdf.pages()?;
    let mut shapes = pages.iter().map(|(_, shape)| *shape).collect::<Vec<_>>();
    if request.mode == Mode::Copies
        && let Some(fd) = std::env::var_os("CANOPA_PRINT_ORIGINAL_PDF_FD")
    {
        let fd: u32 = fd
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid original PDF descriptor"))?
            .parse()?;
        let original = Pdf::open(&File::open(format!("/proc/self/fd/{fd}"))?)?;
        let originals = original.pages()?;
        anyhow::ensure!(
            originals.len() == pages.len(),
            "PDF conversion changed page count"
        );
        for (shape, (_, original)) in shapes.iter_mut().zip(originals) {
            let size = shape.normalized()?.0;
            let exact = original.normalized()?.0;
            anyhow::ensure!(
                size.iter()
                    .zip(exact)
                    .all(|(actual, expected)| (actual - expected).abs() <= 0.02),
                "PDF conversion changed page geometry"
            );
            // Content keeps the converter's coordinate system and appearance
            // placement; sheet sizes (including copy-boundary blanks) use the
            // original physical bounds without a second layout or scaling.
            *shape = original;
        }
    }
    let plan = Plan::new(request, &shapes)?;
    let mut total = 0;
    let mut forms = std::collections::HashMap::new();
    for index in plan.sheets.iter().flat_map(|sheet| &sheet.pages) {
        if let std::collections::hash_map::Entry::Vacant(entry) = forms.entry(*index) {
            entry.insert(pdf.form(pages[*index].0, pages[*index].1, &mut total)?);
        }
    }
    // SAFETY: all new object handles are owned by the same live Pdf. The new
    // catalog deliberately keeps only printable pages/resources, not actions.
    unsafe {
        let tree = (pdf.api.indirect)(data, (pdf.api.dict)(data));
        let kids = (pdf.api.array)(data);
        for sheet in &plan.order {
            let sheet = &plan.sheets[*sheet];
            let resources = (pdf.api.dict)(data);
            let xobjects = (pdf.api.dict)(data);
            let mut content = String::new();
            for (slot, index) in sheet.pages.iter().enumerate() {
                let (clip, matrix) = plan.placement(request, pages[*index].1, slot)?;
                let name = CString::new(format!("/P{index}"))?;
                (pdf.api.set)(data, xobjects, name.as_ptr(), forms[index]);
                content.push_str(&format!(
                    "q {:.12} {:.12} {:.12} {:.12} re W n {} cm /P{index} Do Q\n",
                    clip[0],
                    clip[1],
                    clip[2],
                    clip[3],
                    matrix
                        .0
                        .iter()
                        .map(|v| format!("{v:.12}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            pdf.set(resources, c"/XObject", xobjects);
            let page = (pdf.api.indirect)(data, (pdf.api.dict)(data));
            pdf.set(page, c"/Type", pdf.name(c"/Page"));
            pdf.set(page, c"/Parent", tree);
            pdf.set(page, c"/Resources", resources);
            pdf.set(page, c"/Contents", pdf.stream(content.as_bytes())?);
            pdf.set(
                page,
                c"/MediaBox",
                pdf.parse(array(&[0., 0., sheet.size[0], sheet.size[1]]))?,
            );
            (pdf.api.append)(data, kids, page);
            pdf.check()?;
        }
        pdf.set(tree, c"/Type", pdf.name(c"/Pages"));
        pdf.set(tree, c"/Kids", kids);
        pdf.set(
            tree,
            c"/Count",
            (pdf.api.integer)(data, plan.order.len() as i64),
        );
        // Replace the trailer Root with a fresh catalog so annotations/actions
        // and interactive data from the source cannot escape as active output.
        let root = (pdf.api.dict)(data);
        pdf.set(root, c"/Type", pdf.name(c"/Catalog"));
        pdf.set(root, c"/Pages", tree);
        pdf.set(
            (pdf.api.trailer)(data),
            c"/Root",
            (pdf.api.indirect)(data, root),
        );
        pdf.status((pdf.api.update_pages)(data))?;
        pdf.status((pdf.api.begin_write)(data, c"/proc/self/fd/1".as_ptr()))?;
        (pdf.api.preserve)(data, 0);
        pdf.status((pdf.api.write)(data))?;
    }
    Ok(())
}
