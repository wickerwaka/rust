// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![crate_name = "rustar"]
#![experimental]
#![desc = "rustar, Rust archive processing"]
#![license = "MIT/ASL2"]
#![crate_type = "dylib"]
#![crate_type = "rlib"]
#![crate_type = "bin"]
#![feature(unsafe_destructor)]


extern crate llvm = "rustc_llvm";
extern crate libc;

use llvmhelp::{ObjectFile};
use std::os;
use std::str::{from_utf8};
use std::io::{File, IoError, EndOfFile};

mod llvmhelp;

static MAGIC: &'static [u8] = b"!<arch>\n";
static FMAG: &'static str = "`\n";

static NAME_LEN : uint = 16u;
static DATE_LEN : uint = 12u;
static ID_LEN : uint = 6u;
static MODE_LEN : uint = 8u;
static SIZE_LEN : uint = 10u;

enum Format {
	BSD,
	GNU,
	COFF,
}

/*
pub struct Config {
	format: Option<Format>,
	force_extended_names: bool
}

impl Config {
	fn new() -> Config {
		Config {
			format: None,
			force_extended_names: false
		}
	}
}
*/

#[deriving(Show)]
pub struct Member {
	name: String,
	offset: uint,
	uid: uint,
	gid: uint,
	mode: uint,
	date: uint,
	data: Vec<u8>
}

pub struct Archive {
	format: Format,
	path: Path,
	members: Vec<Member>,
}

#[deriving(Show)]
pub enum ArchiveError {
	ArchiveIoError(IoError, &'static str),
	FormatError(&'static str),
	InvalidString(&'static str),
	InvalidInteger(&'static str),
	NoMoreMembers,
	StringTooLong,
	InvalidMagic,
	InvalidMemberMagic,
	DuplicateStringTable,
	MissingStringTable
}

pub enum NameEncoding {
	BSDSimple(String),
	GNUSimple(String),
	BSDExtended(uint),
	GNUExtended(uint),
}

fn decode_name( name: &str ) -> Result<NameEncoding, ArchiveError> {
	if name.starts_with( "#1/" ) {
		let name_len = try!( parse_uint( name.slice_from(3), 10, "BSD name length" ) );
		return Ok(BSDExtended(name_len));
	}

	match name.rfind( '/' ) {
		Some(0) => {
			match name.char_at(1) {
				' ' => Ok(GNUSimple( "/".to_string() )),
				_ => {
					let name_offset = try!( parse_uint( name.slice_from(1), 10, "GNU name offset" ) );
					Ok(GNUExtended(name_offset))
				}
			}
		},
		Some(idx) => Ok(GNUSimple( name.slice_to(idx).to_string() )),
		None => Ok(BSDSimple( name.trim_right_chars( ' ' ).to_string() )),
	}
}

fn read_string<T: Reader>( reader: &mut T, len: uint, reason: &'static str ) -> Result<String, ArchiveError> {
	let bytes = try!( reader.read_exact( len ).map_err(
			|e| ArchiveIoError( e, reason ) ) );

	match std::str::from_utf8( bytes.as_slice() ) {
		Some(s) => {
			Ok(s.trim_right_chars( '\0' ).into_string())
		},
		None => Err(InvalidString( reason ))
	}
}

fn write_string<T: Writer>( writer: &mut T, len: uint, s: &str ) -> Result<(), ArchiveError> {
	let slen = s.len();
	if slen > len {
		return Err(StringTooLong)
	}

	let mut v : Vec<u8> = Vec::with_capacity( len );
	v.push_all( s.as_bytes() );

	if slen < len {
		v.grow( len - slen, &0x20u8 );
	}

	writer.write( v.as_slice() ).map_err( |e| ArchiveIoError( e, "writing string" ) )
}

fn write_uint<T: Writer>( writer: &mut T, len: uint, radix: uint, v: uint ) -> Result<(), ArchiveError> {
	let s = format!( "{}", std::fmt::radix( v, radix as u8 ) );
	write_string( writer, len, s.as_slice() )
}


fn parse_uint( s: &str, radix: uint, reason: &'static str ) -> Result<uint, ArchiveError> {
	let num_str = s.trim_right_chars( ' ' );
	if num_str.len() == 0 {
		return Ok(0u); // GNU sometimes writes nothing instead of 0
	}
	match std::num::from_str_radix( num_str, radix ) {
		Some(i) => Ok(i),
		None => {
			println!( "{} {}", reason, num_str );
			Err(InvalidInteger( reason ))
		}
	}
}

fn read_uint<T: Reader>( reader: &mut T, len: uint, radix: uint, reason: &'static str ) -> Result<uint, ArchiveError> {
	let s = try!( read_string( reader, len, reason ) );
	parse_uint( s.as_slice(), radix, reason )
}

struct SimpleReader<'a> {
	data: &'a[u8],
}

impl<'a> SimpleReader<'a> {
	fn new( data: &'a[u8] ) -> SimpleReader {
		SimpleReader {
			data: data
		}
	}

	fn read_u32_at( &mut self, offset: uint ) -> Option<u32> {
		if offset + 4 > self.data.len() {
			return None
		}

		let mut result : u32 = 0;

		unsafe {
			let src = self.data.slice_from(offset).as_ptr();
			let dst : *mut u8 = std::intrinsics::transmute( &mut result );
			std::intrinsics::copy_memory( dst, src, 4 );
		};
		Some(result)
	}

	fn read_string_at( &mut self, offset: uint ) -> Option<&str> {
		if offset >= self.data.len() {
			return None;
		}

		let slc = self.data.slice_from(offset);
		
		match slc.position_elem( &0u8 ) {
			Some(x) => {
				std::str::from_utf8( slc.slice_to( x ) )
			},
			None => {
				std::str::from_utf8( slc )
			}
		}
	}

}

fn read_symbol_table_bsd( member: &Member ) -> Result<(), ArchiveError> {
	let mut reader = SimpleReader::new( member.data.as_slice() );

	let ranlib_size = reader.read_u32_at(0).unwrap_or(0) as uint;
	//let table_size = reader.read_u32_at(ranlib_size + 4).unwrap_or(0) as uint;
	let table_offset = ranlib_size + 8u;

	for i in std::iter::range_step(4, ranlib_size + 4, 8) {
		let string_offset = reader.read_u32_at(i).unwrap_or(0) as uint;
		let header_offset = reader.read_u32_at(i+4).unwrap_or(0);
		let sym_name = reader.read_string_at(string_offset + table_offset).unwrap();
		println!( "{} {} {}", string_offset, header_offset, sym_name );
	}

	Ok(())
}


impl Member {
	fn read<T: Reader + Seek>( reader: &mut T, string_table: &Option<Vec<u8>> ) -> Result<Member, ArchiveError> {
		let offset = match reader.tell() {
			Err(x) => return Err(ArchiveIoError(x, "determining offset")),
			Ok(x) => x
		};

		let ar_name = match read_string( reader, NAME_LEN, "member name" ) {
			Err(ArchiveIoError(ref e, _)) if e.kind == std::io::EndOfFile => {
				return Err(NoMoreMembers)
			},
			Err(e) => return Err(e),
			Ok(n) => n
		};

		let date = try!( read_uint( reader, DATE_LEN, 10, "member date" ) );
		let uid = try!( read_uint( reader, ID_LEN, 10, "member uid" ) );
		let gid = try!( read_uint( reader, ID_LEN, 10, "member gid" ) );
		let mode = try!( read_uint( reader, MODE_LEN, 8, "member mode" ) );
		let mut size = try!( read_uint( reader, SIZE_LEN, 10, "member size" ) );
		let fmag = try!( read_string( reader, FMAG.len(), "member magic" ) );

		if fmag.as_slice().ne( &FMAG ) {
			return Err(InvalidMemberMagic);
		}

		let name = match try!( decode_name( ar_name.as_slice() ) ) { 
			BSDSimple(s) => s,
			GNUSimple(s) => s,
			BSDExtended(len) => {
				size -= len;
				try!( read_string( reader, len, "BSD extended name" ) )
			},
			GNUExtended(ofs) => {
				match *string_table {
					None => return Err(MissingStringTable),
					Some(ref tbl) => {
						let slc = tbl.slice_from(ofs);
						let sub = match slc.iter().position( |c| *c == '/' as u8 ) {
							None => slc,
							Some(x) => slc.slice_to(x)
						};
						match std::str::from_utf8( sub ) {
							None => return Err(InvalidString( "in string table" )),
							Some(s) => String::from_str(s),
						}
					}
				}
			}
		};

		let data = match reader.read_exact( size ) {
			Ok(e) => e,
			Err(e) => return Err(ArchiveIoError(e, "reading member data"))
		};

		Ok( Member{
			name: name,
			offset: offset as uint,
			uid: uid,
			gid: gid,
			mode: mode,
			date: date,
			data: data
		} )
	}

	fn write<T: Writer>( &self, writer: &mut T ) -> Result<(), ArchiveError> {
		let name_len = self.name.len();
		let ex_name = format!( "#1/{}", name_len );
		try!( write_string( writer, NAME_LEN, ex_name.as_slice() ) );
		try!( write_uint( writer, DATE_LEN, 10, self.date ) );
		try!( write_uint( writer, ID_LEN, 10, self.uid ) );
		try!( write_uint( writer, ID_LEN, 10, self.gid ) );
		try!( write_uint( writer, MODE_LEN, 8, self.mode ) );
		try!( write_uint( writer, SIZE_LEN, 10, self.data.len() + name_len ) );
		try!( write_string( writer, FMAG.len(), FMAG ) );
		try!( write_string( writer, name_len, self.name.as_slice() ) );

		writer.write( self.data.as_slice() ).map_err(
			|err| ArchiveIoError( err, "writing data" )
		)
	}
}
		

impl Archive {
	pub fn open(path: &Path) -> Result<Archive, ArchiveError> {
		let mut f = File::open( path );
		Archive::read( &mut f, path )
	}

	pub fn read<T: Reader + Seek>(reader: &mut T, path: &Path) -> Result<Archive, ArchiveError> {
		let magic = try!(
			reader.read_exact( MAGIC.len() ).map_err(
				|err| ArchiveIoError( err, "reading magic" )
				)
			);

		if magic.as_slice().ne( &MAGIC ) {
			return Err( InvalidMagic );
		}

		let mut members = Vec::new();
		let mut string_table = None;

		loop {
			let member = match Member::read( reader, &string_table ) {
				Err(NoMoreMembers) => break,
				Err(e) => return Err(e),
				Ok(m) => m
			};

			if member.name.as_slice() == "/" {
				match string_table {
					None => string_table = Some( member.data.clone() ),
					Some(_) => return Err(DuplicateStringTable)
				};
			}

			if member.name.as_slice().starts_with( "__.SYMDEF" ) {
				try!( read_symbol_table_bsd( &member ) );
			}

			members.push( member );
		}

		Ok( Archive{ path: path.clone(), members: members, format: BSD } )
	}
	
	pub fn write(&self) -> Result<(), ArchiveError> {
		let mut f = File::create( &self.path.with_filename( "test.out" ) );

		try!(
			f.write( MAGIC ).map_err(
				|err| ArchiveIoError( err, "writing magic" )
				)
			);

		for member in self.members.iter() {
			try!( member.write( &mut f ) );
		}

		Ok(())
	}
}


fn read_symbols( member: &Member ) {
	match ObjectFile::new( member.data.as_slice(), member.name.as_slice() ) {
		None => return,
		Some(obj) => {
			for sym in obj.symbols() {
				println!( "{}: {} {}", member.name, sym.name, sym.address );
			}
		}
	}
}

fn main() {
	let args = os::args();
	let path = &Path::new( args[1].clone() );
	match Archive::open( path ) {
		Err(x) => println!( "Error reading archive {}: {}", path.display(), x ),
		Ok(x) => {
			for m in x.members.iter() {
				if m.name.as_slice().ends_with( ".o" ) {
					read_symbols( m );
				}
			}
			match x.write() {
				Ok(_) => {},
				Err(e) => println!( "{}", e )
			}
		}
	}
}


	

