#!/usr/bin/env python3
"""Verify explicit source-overlay additions against original archive contents."""
import difflib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import vendor_sources as sources


class Additions(unittest.TestCase):
    def fixture(self, directory, original=False, extra=False, notice=False):
        root=Path(directory)/'root';archives=Path(directory)/'archives'
        (root/'vendor').mkdir(parents=True);archives.mkdir()
        license=b'MIT fixture\n';(root/'vendor/LICENSE').write_bytes(license)
        archive=archives/'demo.tar.gz'
        files={'LICENSE':license,'src/existing.rs':b'original\n'}
        if notice:
            files['NOTICE']=b'Upstream attribution notice\n'
            (root/'vendor/NOTICE').write_bytes(files['NOTICE'])
        if original:files['src/new.rs']=b'previously present\n'
        with tarfile.open(archive,'w:gz') as output:
            for path,content in files.items():
                info=tarfile.TarInfo('demo/'+path);info.size=len(content);output.addfile(info,io.BytesIO(content))
        content=b'pub const ADDED: bool = true;\n'
        patch='diff --git a/src/new.rs b/src/new.rs\n'+''.join(difflib.unified_diff([],content.decode().splitlines(keepends=True),fromfile='/dev/null',tofile='b/src/new.rs'))
        if extra:patch+=''.join(difflib.unified_diff([],['surprise\n'],fromfile='/dev/null',tofile='b/src/surprise.rs'))
        (root/'vendor/demo.patch').write_text(patch)
        record={'sources':{'demo':{'archive':archive.name,'archive_sha256':sources.digest(archive.read_bytes()),
                    'archive_root':'demo','license':{'path':'vendor/LICENSE','upstream_path':'LICENSE','sha256':sources.digest(license)}}},
                'patches':[{'source':'demo','path':'vendor/demo.patch','sha256':sources.digest(patch.encode()),
                    'files':{'src/new.rs':{'upstream':None,'patched':sources.digest(content)}}}]}
        if notice:
            record['sources']['demo']['notices']=[{'path':'vendor/NOTICE','upstream_path':'NOTICE',
                'sha256':sources.digest(files['NOTICE'])}]
        (root/'vendor/provenance.json').write_text(json.dumps({'source_overlays':{'demo':record}}))
        return root,archives

    def test_absent_upstream_is_explicit_and_archive_patch_materialization_matches(self):
        with tempfile.TemporaryDirectory() as directory:
            root,archives=self.fixture(directory)
            self.assertEqual(sources.check(root,archives),[])
            prepared=sources.prepare(root,archives,Path(directory)/'prepared','demo')
            self.assertEqual((prepared['demo']/'src/new.rs').read_text(),'pub const ADDED: bool = true;\n')

    def test_null_hash_cannot_hide_an_existing_upstream_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root,archives=self.fixture(directory,original=True)
            self.assertTrue(any('upstream checksum mismatch' in error for error in sources.check(root,archives)))

    def test_extracted_source_matches_actual_materialization_including_single_line_hunks(self):
        with tempfile.TemporaryDirectory() as directory:
            root,archives=self.fixture(directory)
            record=sources.records(root)['demo']
            source=sources.added_file(root,record,'src/new.rs')
            prepared=sources.prepare(root,archives,Path(directory)/'prepared','demo')
            self.assertEqual(source,(prepared['demo']/'src/new.rs').read_bytes())
            record['patches'][0]['files']['src/new.rs']['upstream']='0'*64
            with self.assertRaises(ValueError):sources.added_file(root,record,'src/new.rs')

    def test_extraction_refuses_an_extra_line_outside_the_declared_hunk(self):
        with tempfile.TemporaryDirectory() as directory:
            root,_=self.fixture(directory)
            record=sources.records(root)['demo']
            path=root/'vendor/demo.patch'
            path.write_text(path.read_text()+'+pub const UNDECLARED: bool = true;\n')
            with self.assertRaises(ValueError):sources.added_file(root,record,'src/new.rs')

    def test_an_undeclared_added_file_still_fails_exact_patch_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            root,archives=self.fixture(directory,extra=True)
            self.assertTrue(any('Unexpected patch scope' in error for error in sources.check(root,archives)))

    def test_additional_notice_is_verified_and_materialized(self):
        with tempfile.TemporaryDirectory() as directory:
            root,archives=self.fixture(directory,notice=True)
            self.assertEqual(sources.check(root,archives),[])
            prepared=sources.prepare(root,archives,Path(directory)/'prepared','demo')
            self.assertEqual((prepared['demo']/'NOTICE').read_bytes(),(root/'vendor/NOTICE').read_bytes())

    def test_tampered_local_notice_fails_without_an_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root,_=self.fixture(directory,notice=True)
            (root/'vendor/NOTICE').write_text('Replaced attribution\n')
            self.assertTrue(any('licence differs' in error and 'NOTICE' in error for error in sources.check(root)))

    def test_rehashed_archive_cannot_replace_the_declared_notice(self):
        with tempfile.TemporaryDirectory() as directory:
            root,archives=self.fixture(directory,notice=True)
            archive=archives/'demo.tar.gz'
            with tarfile.open(archive) as contents:
                files={m.name:contents.extractfile(m).read() for m in contents if m.isfile()}
            files['demo/NOTICE']=b'Replaced upstream attribution\n'
            with tarfile.open(archive,'w:gz') as output:
                for name,content in files.items():
                    info=tarfile.TarInfo(name);info.size=len(content);output.addfile(info,io.BytesIO(content))
            provenance=json.loads((root/'vendor/provenance.json').read_text())
            provenance['source_overlays']['demo']['sources']['demo']['archive_sha256']=sources.digest(archive.read_bytes())
            (root/'vendor/provenance.json').write_text(json.dumps(provenance))
            self.assertTrue(any('licence does not match upstream' in error and 'NOTICE' in error
                                for error in sources.check(root,archives)))


if __name__=='__main__':unittest.main()
