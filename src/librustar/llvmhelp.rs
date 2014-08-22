use llvm;
use libc::{size_t, c_char};
use std::string::raw::{from_buf};


pub struct ObjectFile<'a> {
	pub llof: llvm::ObjectFileRef
}


impl<'a> ObjectFile<'a> {
	pub fn new<'a>( data: &'a[u8], name: &str ) -> Option<ObjectFile<'a>> {
		let buf = data.as_ptr() as *const c_char;
		let lbuf = name.with_c_str( |name| unsafe {
			llvm::LLVMCreateMemoryBufferWithMemoryRange( buf, data.len() as size_t, name, 0u32 )
		} );
		
		if lbuf as int == 0 {
			return None
		}

		let llof = unsafe{ llvm::LLVMCreateObjectFile( lbuf ) };
		if llof as int == 0 {
			return None
		}

		Some( ObjectFile {
			llof: llof
		} )
	}

	pub fn symbols(&self) -> Symbols {
		unsafe {
			Symbols {
				obj: self,
				first: true,
				sym: llvm::LLVMGetSymbols( self.llof )
			}
		}
	}
}

#[unsafe_destructor]
impl<'a> Drop for ObjectFile<'a> {
	fn drop(&mut self) {
		unsafe {
			llvm::LLVMDisposeObjectFile( self.llof );
		}
	}
}

pub struct Symbol {
	pub name: String,
	pub address: u64,
	pub size: u64,
	pub flags: llvm::SymbolFlags
}


pub struct Symbols<'a> {
	obj: &'a ObjectFile<'a>,
	sym: llvm::SymbolIteratorRef,
	first: bool
}

impl<'a> Iterator<Symbol> for Symbols<'a> {
	fn next(&mut self) -> Option<Symbol> {
		if !self.first {
			unsafe{ llvm::LLVMMoveToNextSymbol( self.sym ); }
		}
		self.first = false;
		
		unsafe {
			if llvm::LLVMIsSymbolIteratorAtEnd( self.obj.llof, self.sym ) == llvm::True {
				return None;
			}
			let namebuf = llvm::LLVMGetSymbolName( self.sym ) as *const u8;
			let name = from_buf( namebuf );
			return Some( Symbol{
				name: name,
				address: llvm::LLVMGetSymbolAddress( self.sym ),
				size: llvm::LLVMGetSymbolSize( self.sym ),
				flags: llvm::LLVMRustGetSymbolFlags( self.sym )
			} );
		}
	}
}


