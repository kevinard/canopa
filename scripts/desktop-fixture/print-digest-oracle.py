"""Strict byte-level Digest fixture, independent of CUPS and urllib parsing.

RFC9110 section5.6.4 quoted-pair and RFC7616 sections3.4/3.4.5.
Never retain an Authorization header or a computed response in receipts.
"""
import base64, hashlib, hmac

TOKEN=frozenset(b"!#$%&'*+-.^_`|~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz")
def octets(value):
    return value.encode('utf-8') if isinstance(value,str) else bytes(value)

def quote(value):
    value=octets(value)
    assert all(c==9 or c>=32 and c!=127 for c in value)
    return b'"'+value.replace(b'\\',b'\\\\').replace(b'"',b'\\"')+b'"'

def fields(header):
    data=header.encode('latin1') if isinstance(header,str) else bytes(header)
    if data[:7].lower()!=b'digest ':raise ValueError('scheme')
    data=data[7:];i=0;values={};quoted=set()
    def whitespace():
        nonlocal i
        while i<len(data) and data[i] in (9,32):i+=1
    def token():
        nonlocal i
        start=i
        while i<len(data) and data[i] in TOKEN:i+=1
        if start==i:raise ValueError('token')
        return data[start:i]
    whitespace()
    while i<len(data):
        name=token().lower();whitespace()
        if i>=len(data) or data[i]!=61:raise ValueError('equals')
        i+=1;whitespace()
        if i<len(data) and data[i]==34:
            quoted.add(name);i+=1;value=bytearray()
            while True:
                if i>=len(data):raise ValueError('unterminated')
                c=data[i];i+=1
                if c==34:break
                if c==92:
                    if i>=len(data):raise ValueError('escape')
                    c=data[i];i+=1
                if c!=9 and (c<32 or c==127):raise ValueError('control')
                value.append(c)
            value=bytes(value)
        else:value=token()
        if name in values:raise ValueError('duplicate')
        values[name]=value;whitespace()
        if i==len(data):break
        if data[i]!=44:raise ValueError('separator')
        i+=1;whitespace()
        if i==len(data):raise ValueError('trailing-comma')
    if not values:raise ValueError('empty')
    return values,quoted

class Verifier:
    def __init__(self,user,password,algorithm='SHA-256',realm=b'Canopa private probe',nonce=b'probe-nonce',opaque=b'probe-opaque'):
        self.user=octets(user);self.password=octets(password);self.algorithm=algorithm
        self.realm=octets(realm);self.nonce=octets(nonce);self.opaque=None if opaque is None else octets(opaque);self.seen=set()
        assert algorithm in ['Basic','MD5','SHA-256','legacy']
    def challenge(self):
        if self.algorithm=='Basic':return ('Basic realm='+quote(self.realm).decode('latin1'))
        params=[b'realm='+quote(self.realm),b'nonce='+quote(self.nonce)]
        if self.opaque is not None:params.append(b'opaque='+quote(self.opaque))
        if self.algorithm!='legacy':params.extend([b'algorithm='+self.algorithm.encode(),b'qop="auth"',b'charset=UTF-8'])
        data=b'Digest '+b', '.join(params)
        parsed,_=fields(data);assert parsed[b'realm']==self.realm and parsed[b'nonce']==self.nonce
        if self.opaque is not None:assert parsed[b'opaque']==self.opaque
        return data.decode('latin1')
    def authorize(self,header,method,resource):
        row=dict(present=header is not None,accepted=False)
        if header is None:return False,row
        if self.algorithm=='Basic':
            try:
                raw=header.encode('latin1');assert raw[:6].lower()==b'basic '
                pair=base64.b64decode(raw[6:],validate=True)
                row['accepted']=hmac.compare_digest(pair,self.user+b':'+self.password)
            except (ValueError,AssertionError):row['reason']='basic-frame'
            return row['accepted'],row
        try:
            values,quoted=fields(header)
            if not {b'username',b'realm',b'nonce',b'uri',b'response'}.issubset(quoted):raise ValueError('required-quoted')
            if quoted.intersection({b'algorithm',b'qop',b'nc'}):raise ValueError('required-token')
            row.update(user_matches=values[b'username']==self.user,realm_matches=values[b'realm']==self.realm,nonce_matches=values[b'nonce']==self.nonce,uri_matches=values[b'uri']==octets(resource))
            if not all(row[k] for k in ['user_matches','realm_matches','nonce_matches','uri_matches']):return False,row
            if self.opaque is not None and (values.get(b'opaque')!=self.opaque or b'opaque' not in quoted):raise ValueError('opaque')
            algorithm='MD5' if self.algorithm=='legacy' else self.algorithm
            if values.get(b'algorithm',b'MD5').upper()!=algorithm.encode():raise ValueError('algorithm')
            digest=lambda data:hashlib.new('md5' if algorithm=='MD5' else 'sha256',data).hexdigest().encode()
            a1=digest(self.user+b':'+self.realm+b':'+self.password);a2=digest(octets(method)+b':'+octets(resource))
            if self.algorithm=='legacy':
                if b'qop' in values:raise ValueError('legacy-qop')
                expected=digest(a1+b':'+self.nonce+b':'+a2);replay=None
            else:
                if values.get(b'qop')!=b'auth' or b'cnonce' not in quoted:raise ValueError('qop')
                nc=values[b'nc'];cnonce=values[b'cnonce']
                if len(nc)!=8 or not all(c in b'0123456789abcdefABCDEF' for c in nc) or not int(nc,16) or not cnonce:raise ValueError('nonce-count')
                expected=digest(a1+b':'+self.nonce+b':'+nc+b':'+cnonce+b':auth:'+a2)
                replay=(self.user,self.nonce,cnonce,int(nc,16))
            row['response_correct']=hmac.compare_digest(values[b'response'],expected)
            if not row['response_correct']:return False,row
            if replay is not None and replay in self.seen:raise ValueError('replayed-response')
            self.seen.add(replay);row['accepted']=True;return True,row
        except (KeyError,ValueError) as error:
            return False,dict(row,reason=str(error))

def check_reference():
    # Exact published vectors, not values generated by the oracle itself.
    vectors=[
      ('MD5',b'testrealm@host.com',b'Circle Of Life',b'dcd98b7102dd2f0e8b11d0f600bfb0c093',b'0a4f113b',None,b'6629fae49393a05397450978507c4ef1'),
      ('MD5',b'http-auth@example.org',b'Circle of Life',b'7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v',b'f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ',b'FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS',b'8ca523f5e9506fed4657c9700eebdbec'),
      ('SHA-256',b'http-auth@example.org',b'Circle of Life',b'7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v',b'f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ',b'FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS',b'753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1')]
    for algorithm,realm,password,nonce,cnonce,opaque,response in vectors:
        parts=[b'username="Mufasa"',b'realm='+quote(realm),b'nonce='+quote(nonce),b'uri="/dir/index.html"',b'qop=auth',b'nc=00000001',b'cnonce='+quote(cnonce),b'response='+quote(response),b'algorithm='+algorithm.encode()]
        if opaque:parts.append(b'opaque='+quote(opaque))
        header=b'Digest '+b', '.join(parts);v=Verifier(b'Mufasa',password,algorithm,realm,nonce,opaque)
        assert v.authorize(header.decode('latin1'),'GET','/dir/index.html')[0],algorithm
        assert not v.authorize(header.decode('latin1'),'GET','/dir/index.html')[0]
    assert fields(b'Digest realm="a\\"b\\\\c, d"')[0]=={b'realm':b'a"b\\c, d'}
    assert quote(b'a"b\\c, d')==b'"a\\"b\\\\c, d"'
    for invalid in [b'Digest x="unterminated',b'Digest x="bad"tail',b'Digest x="bad\r\nvalue"',b'Digest x=a, X=b',b'Digest x=a,',b'Digest x=',b'Digest x="bad\\']:
        try:fields(invalid)
        except ValueError:continue
        raise AssertionError('Malformed field accepted')
    for scheme in ['Basic', 'basic']:
        v=Verifier(b'person',b'private','Basic')
        assert v.authorize(scheme+' cGVyc29uOnByaXZhdGU=','POST','/')[0]
    return dict(reference_vectors=len(vectors),golden_quoted_pairs=2,malformed_cases=7,basic_scheme_case_checks=2)

if __name__=='__main__':print(check_reference())
