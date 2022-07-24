
pub mod io {
	use std::fs::File;
	use std::io::Read;
use std::path::Path;

	pub fn read_bin_file(loc: &str) -> Vec<u8> {
		let path = Path::new(loc);
		let mut file = File::open(&path).expect("file open failure");
		let mut buffer = Vec::new();

		file.read_to_end(&mut buffer).expect("failed to read file");
		return buffer;
	}	
}