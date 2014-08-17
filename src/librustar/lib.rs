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

use std::os;
use std::str::{from_utf8};
use std::io::{File, IoError};

static MAGIC: &'static [u8] = b"!<arch>\n";
static FMAG: &'static str = "`\n";

static NAME_LEN : uint = 16u;
static DATE_LEN : uint = 12u;
static ID_LEN : uint = 6u;
static MODE_LEN : uint = 8u;
static SIZE_LEN : uint = 10u;


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
}

fn read_string<T: Reader>( reader: &mut T, len: uint, reason: &'static str ) -> Result<String, ArchiveError> {
	let bytes = try!( reader.read_exact( len ).map_err(
			|e| ArchiveIoError( e, reason ) ) );

	match String::from_utf8( bytes ) {
		Ok(s) => Ok(s),
		Err(_) => Err(InvalidString( reason ))
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
	let num_str = s.splitn( 1, ' ' ).nth(0).unwrap();
	if num_str.len() == 0 {
		return Ok(0u);
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

impl Member {
	fn read<T: Reader + Seek>( reader: &mut T ) -> Result<Member, ArchiveError> {
		let offset = match reader.tell() {
			Err(x) => return Err(ArchiveIoError(x, "determining offset")),
			Ok(x) => x
		};

		let mut name = match read_string( reader, NAME_LEN, "member name" ) {
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

		if name.as_slice().starts_with( "#1/" ) {
			let name_len = try!( parse_uint( name.as_slice().slice_from(3), 10, "extended name length" ) );
			name = try!( read_string( reader, name_len, "extended name" ) );
			size -= name_len;
		}

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

		let magic = try!(
			f.read_exact( MAGIC.len() ).map_err(
				|err| ArchiveIoError( err, "reading magic" )
				)
			);

		if magic.as_slice().ne( &MAGIC ) {
			return Err( InvalidMagic );
		}

		let mut members = Vec::new();

		loop {
			let member = match Member::read( &mut f ) {
				Err(NoMoreMembers) => break,
				Err(e) => return Err(e),
				Ok(m) => m
			};
			members.push( member );
		}

		Ok( Archive{ path: path.clone(), members: members } )
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

fn main() {
	let args = os::args();
	let path = &Path::new( args[1].clone() );
	match Archive::open( path ) {
		Err(x) => println!( "Error reading archive {}: {}", path.display(), x ),
		Ok(x) => {
			for m in x.members.iter() {
				println!( "{}: {}", m.name, m.data.len() );
			}
			match x.write() {
				Ok(_) => {},
				Err(e) => println!( "{}", e )
			}
		}
	}
}


	

