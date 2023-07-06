use glam::{Vec3, Vec4, UVec4};

const OCTANT_COUNT: usize = 8;

type DagAddress = u32;

macro_rules! set_derives_stores {
	($($structs:item),*) => {
		$(
			#[repr(C)]
			#[derive(Clone, Debug)]
			#[allow(dead_code)]
			$structs
		)* 
	};
}

set_derives_stores!(
	struct Octant {
		index: DagAddress, //index of the next node
		colour: u32, //rgba
		normal: u32, //xyz desnity //change name to volume, or physical
		extra: u32, // 8 shine, 8 radiance, 16 or 8 frames, 
	},
	struct Node {
		octants: [Octant; OCTANT_COUNT],
	},
	pub struct GPUOctDag {
		graph_vec: Vec<Node>,
	}
);

impl From<&GPUOctDag> for &[u8] {
	fn from(dag: &GPUOctDag) -> Self {
		unsafe{std::slice::from_raw_parts(dag.graph_vec[..].as_ptr() as *const u8,
			std::mem::size_of::<Node>() * dag.graph_vec.len())}
	}
}
/* 
impl From<T> for OctDag {
	fn from(value: T) -> Self {
	} 
}
*/

macro_rules! set_derives_uniforms {
	($($structs:item),*) => {
		$(
			#[repr(C)]
			//#[repr(C, align(8))]
			#[derive(Clone, Copy, Debug)]
			#[allow(dead_code)]
			$structs
		)* 
	};
}
set_derives_uniforms!(
	pub struct TemporalInputData {
		pub temporals: UVec4, //time, frame, time delta, frame delta
	},
	pub struct ViewInputData {
		pub pos: Vec4, //x, y, z, pad
		pub rads: Vec4, //yaw, pitch, roll, pad(change to fov)
	},
	pub struct LightInputData {
		pub pos: Vec4,//x y z pad
		pub dir: Vec4,//x, y, z, fov 
		pub rgb: Vec4,//r g b pad //a could be used to increase itensity?
	},

	/*
	 there structs are required, 
	 used to size allocation of data in the device
	 */
	pub struct ViewData {
		pos: Vec3,
		len: f32,
		rgba: u32, 		
		normal: u32, 
	},
	//find some form of direction that does not reqeat ie (1, 1), (2, 2)
	pub struct LightData {
		rgb: f32, //rgb, parellax?
		direction: u32, //xyz, dist?
	},
	pub struct LightVolume {
		data: [LightData; 4],
	}
);