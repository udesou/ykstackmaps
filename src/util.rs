// Copyright (c) 2018 King's College London
// Created by the Software Development Team <http://soft-dev.org/>
//
// The Universal Permissive License (UPL), Version 1.0
//
// Subject to the condition set forth below, permission is hereby granted to any
// person obtaining a copy of this software, associated documentation and/or
// data (collectively the "Software"), free of charge and under any and all
// copyright rights in the Software, and any and all patent rights owned or
// freely licensable by each licensor hereunder covering either (i) the
// unmodified Software as contributed to or provided by such licensor, or (ii)
// the Larger Works (as defined below), to deal in both
//
// (a) the Software, and
// (b) any piece of software and/or hardware listed in the lrgrwrks.txt file
// if one is included with the Software (each a "Larger Work" to which the
// Software is contributed by such licensors),
//
// without restriction, including without limitation the rights to copy, create
// derivative works of, display, perform, and distribute the Software and make,
// use, sell, offer for sale, import, export, have made, and have sold the
// Software and the Larger Work(s), and to sublicense the foregoing rights on
// either these or other terms.
//
// This license is subject to the following condition: The above copyright
// notice and either this complete permission notice or at a minimum a reference
// to the UPL must be included in all copies or substantial portions of the
// Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use elf;
use mach_object;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use {SMParserError, SMParserResult};

const STACKMAP_SECTION_NAME: &str = ".llvm_stackmaps";
const STACKMAP_SECTION_NAME_MACO: &str = "__llvm_stackmaps";

pub(crate) fn cursor_from_elf(
    elf_file: &elf::File,
    start_pos: u64,
) -> SMParserResult<Cursor<&Vec<u8>>> {
    let sec_res = elf_file.get_section(STACKMAP_SECTION_NAME);

    if let Some(sec) = sec_res {
        let mut cursor = Cursor::new(&sec.data);
        cursor.seek(SeekFrom::Start(start_pos))?;
        Ok(cursor)
    } else {
        Err(SMParserError::Other(String::from(
            "Can't find stackmap section in binary",
        )))
    }
}

#[derive(Clone, Debug)]
pub struct MacOFile {
    pub buf: Vec<u8>,
}

pub(crate) fn cursor_from_maco(
    mut maco_file: &MacOFile,
    start_pos: u64,
    is_abs_pos: bool,
) -> SMParserResult<Cursor<&Vec<u8>>> {
    use mach_object::{LoadCommand, MachCommand, OFile, CPU_TYPE_X86_64};

    let mut cur = Cursor::new(&maco_file.buf);

    if let OFile::MachFile {
        ref header,
        ref commands,
    } = OFile::parse(&mut cur).unwrap()
    {
        assert_eq!(header.cputype, CPU_TYPE_X86_64);
        assert_eq!(header.ncmds as usize, commands.len());

        for &MachCommand(ref cmd, cmdsize) in commands {
            if let &LoadCommand::Segment64 {
                ref segname,
                ref sections,
                ..
            } = cmd
            {
                for ref sect in sections {
                    if sect.sectname == STACKMAP_SECTION_NAME_MACO {
                        if !is_abs_pos {
                            cur.seek(SeekFrom::Start(sect.offset as u64 + start_pos))?;
                        } else {
                            cur.seek(SeekFrom::Start(start_pos))?;
                        }

                        return Ok(cur);
                    }
                }
            }
        }
    }

    Err(SMParserError::Other(String::from(
        "Can't find stackmap section in binary",
    )))
}

/// Skip the cursor forward the specified number of bytes.
pub(crate) fn cursor_skip(cursor: &mut Cursor<&Vec<u8>>, bytes: i64) -> io::Result<u64> {
    cursor.seek(SeekFrom::Current(bytes))
}

/// Align the cursor to the next 8-byte boundary.
pub(crate) fn cursor_align8(cursor: &mut Cursor<&Vec<u8>>) -> io::Result<u64> {
    let pad = (8 - (cursor.position() % 8)) % 8;
    cursor_skip(cursor, pad as i64)
}

/// A macro to assist in early returns of `Some<Err>` in `Iterator::next()`
/// implementations.
macro_rules! itry {
    ($x:expr) => {{
        let res = $x;
        match res {
            Ok(v) => v,
            Err(e) => return Some(Err(SMParserError::from(e))),
        }
    }};
}
