use std::{mem::size_of};
extern crate glam;
use glam::{Vec3, Vec4, IVec3, Vec4Swizzles, i32::ivec3};

const NULL_INDEX: u32 = 0xFFFFFFFF;
const _POSITIVE_X: u32 = 0b001;
const _POSITIVE_Y: u32 = 0b010;
const _POSITIVE_Z: u32 = 0b100;
const OCTANT_COUNT: usize = 8;
const OCTANT_LIST: [IVec3; 8] = 
	[ivec3(-1, -1, -1),
	ivec3(1, -1, -1),
	ivec3(-1, 1, -1),
	ivec3(1, 1, -1),
	ivec3(-1, -1, 1),
	ivec3(1, -1, 1),
	ivec3(-1, 1, 1),
	ivec3(1, 1, 1),];
const MASK_8BIT: u32 = 0x000000FF;


#[repr(C)]
#[derive(Clone, Copy, Debug, Hash)]
struct NewNode {
	pub tree_indices: [u32; 8],
	pub volume_index: u32, 
}
struct Volume<T> {
	pub volume: [T; 8],
}
struct NewOctant {
	pub colour: u32, //rgba
	pub normal: u32, //xyz desnity //change name to volume, or physical
	pub extra: u32, // 8 shine, 8 radiance, 16 or 8 frames, 
}
struct Dag<T> {
	pub nodes: Vec<NewNode>,
	pub volumes: Vec<Volume<T>>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Hash)]
pub struct Octant {
	pub index: u32, //index of the next node
	pub colour: u32, //rgba
	pub normal: u32, //xyz desnity //change name to volume, or physical
	pub extra: u32, // 8 shine, 8 radiance, 16 or 8 frames, 
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Hash)]
pub struct Node {
	pub octants: [Octant; OCTANT_COUNT],
}
pub struct OctDag {
	pub nodes: Vec<Node>,
}

pub enum TestDagType {
	Box,
	Pillar,
}

#[derive(Clone, Copy)]
pub enum VolumeType{
	Perimiter,
	Plane,
	Pillar,
}
#[derive(Clone, Copy)]
pub enum ColourType {
	TiledSpectrum,
	RedZGradient,
	ClearBlue, 
	ColouredWalls,
}

//TODO: consider making fn that creates u8 representation of data for render
//TODO: consider making it possible to generate each virst octant in seperate threads
impl OctDag {
	pub fn new_test(dag_type: TestDagType, max_depth: u32) -> Self {
		if max_depth > 16  {
			panic!("depth out of bounds");
		}
		
		let mut dag = OctDag{nodes: Vec::<Node>::new()};
		let mut level_list = Vec::<Vec::<u32>>::new();
		let vol_list = dag_type.new(max_depth);
		level_list.resize((max_depth) as usize, Vec::<u32>::new());
		dag.nodes.push(Node::new());

		let next_level_size = i32::pow(2, max_depth - 1);
		for i in 0..8 {
			dag.nodes[0].octants[i] = dag.fill_oct( vol_list,
				&mut level_list,  OCTANT_LIST[i] * IVec3::splat(next_level_size), 1, max_depth); 
		}

		return dag;
	}


	fn fill_oct(&mut self, volumes: &[(VolumeType, ColourType)], 
		level_list: &mut Vec<Vec<u32>>, 
		pos: IVec3, 
		depth: u32, 
		max_depth: u32) -> Octant {
		
		let level_size = i32::pow(2, max_depth - depth);
		let max_level_size = i32::pow(2, max_depth);
		let mut octant = Octant::new();
		let mut node = Node::new();

		/*
		run through the volume functions
		find the deepest and store it and the associated colour function
		 */
		let mut funcs = volumes[0];
		let mut max_vol = volumes[0].0.new(pos.as_vec3(), max_level_size as f32);
		for i in volumes {
			let vol = i.0.new(pos.as_vec3(), max_level_size as f32);
			if vol.x < max_vol.x {
				funcs = *i;
				max_vol = vol;
			}
		}
		
		//figuring out how far a point is inside a volume in relation to the level size
		//remeber that x holds the distance inside, while the remainder is the normal
		max_vol.x = max_vol.x / level_size as f32;

		if max_vol.x < -1.0 || (depth == max_depth && max_vol.x <= 1.0) {
			//calling colour function
			octant = funcs.1.new(pos, max_level_size);

			let norm_len = max_vol.yzw().length();
			max_vol.y /= norm_len;
			max_vol.z /= norm_len;
			max_vol.w /= norm_len;

			//max_vol.yzw() = max_vol.yzw().normalize_or_zero();

			octant.normal = pack_f32_u32(Vec4::new(max_vol.y, max_vol.z, max_vol.w, 1.0));

		} else if max_vol.x <= 1.0  {
			let next_depth = depth + 1;
			let next_level_size = i32::pow(2, max_depth - next_depth);
			for i in 0..8 {
				node.octants[i] = self.fill_oct(volumes, level_list, pos + OCTANT_LIST[i] * IVec3::splat(next_level_size), next_depth, max_depth);
			}

			match level_list[depth as usize].iter().find(|x| {
				let mut is_same = true;
				for i in 0..OCTANT_COUNT {
					if node.octants[i].index == NULL_INDEX {
						if is_same && !(self.nodes[**x as usize].octants[i].index == NULL_INDEX && self.nodes[**x as usize].octants[i].colour == node.octants[i].colour) {
							is_same = false;
						}
					} else {
						if is_same && !(self.nodes[**x as usize].octants[i].index == node.octants[i].index) {
							is_same = false;
						}
					}
				}
				return is_same; 
			}) {
				Some(x) => {
					octant.index = *x;
				}
				None => {
					self.nodes.push(node);
					level_list[depth as usize].push(self.nodes.len() as u32 - 1);
					octant.index = self.nodes.len() as u32 - 1;
				}
			}

			let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
			let (mut x, mut y, mut z, mut density) = (0.0, 0.0, 0.0, 0.0);

			/*
			consider changing so that when something like a flat plane has a see through alpha, it is reduced
			currently as walls go up in level they keep their exact alpha, but keep they get thicker, meaning transmissability comes down
			 */
			let filter = |a: usize, b: usize,  arr: &mut [Octant; 8]| {
				if arr[a].normal & MASK_8BIT > arr[b].normal & MASK_8BIT {
					let temp = arr[a];
					arr[a] = arr[b];
					arr[b] = temp;
				}
			};
			let mut octants_copy = node.octants.clone();
			for i in 0..4 {
				filter(i * 2, i * 2 + 1, &mut octants_copy);	
			}
			for i in 0..2 {
				filter(i * 4, i * 4 + 2, &mut octants_copy);
			}
			for i in 0..2 {
				filter(i * 4 + 1, i * 4 + 3, &mut octants_copy);
			}
			for i in 0..2 {
				filter(i * 4 + 2, i * 4 + 3, &mut octants_copy);
				density += (octants_copy[i * 4 + 2].normal & MASK_8BIT) as f32 / 255.0 + (octants_copy[i * 4 + 3].normal & MASK_8BIT) as f32 / 255.0;
			}

			let mut cummulative_denisty = 0.0;
			for i in 0..OCTANT_COUNT {
				let colour = unpack_f32_u32(node.octants[i].colour);
				let normal = unpack_f32_u32(node.octants[i].normal);
				cummulative_denisty += normal.w;
				r += colour.x * normal.w;
				g += colour.y * normal.w;
				b += colour.z * normal.w;
				a += colour.w * normal.w;

				x += normal.x * normal.w;
				y += normal.y * normal.w;
				z += normal.z * normal.w;
			}
			octant.colour = pack_f32_u32(Vec4::new(r / cummulative_denisty, g / cummulative_denisty, b / cummulative_denisty, a / cummulative_denisty));
			octant.normal = pack_f32_u32(Vec4::new(x / cummulative_denisty, y / cummulative_denisty, z / cummulative_denisty, density));
		}
		return octant;
	}
	
	pub fn print_structure(&self, index: usize) {
		print!("index: {}, octants: (", index);
		for i in 0..OCTANT_COUNT {
			print!("[index: {}, colour: {:#08x}, normal: {:#08x}]", self.nodes[index].octants[i].index, self.nodes[index].octants[i].colour, self.nodes[index].octants[i].normal)
		} print!(") \n");
		for i in 0..OCTANT_COUNT {
			if self.nodes[index].octants[i].index != NULL_INDEX {
				self.print_structure(self.nodes[index].octants[i].index as usize);
			}
		}
	}
	pub fn print_size(&self) {
		println!("Node count: {},  Size in Mb: {}", self.nodes.len(), (self.nodes.len() * size_of::<Node>()) as f32 / 1000000.0)
	}
}

impl Octant {
	pub fn difference(&self, other: &Self) -> i32 {
		//if self.index == other.index {0}
		todo!()
	}
}



impl TestDagType {
	pub fn new(&self, depth: u32) -> &[(VolumeType, ColourType)] {
		match self {
			TestDagType::Box => {
				if depth < 2 {
					panic!();
				}
				&[(VolumeType::Perimiter, ColourType::ColouredWalls)]
			},
			TestDagType::Pillar => {
				if depth < 4 {
					panic!();
				}
				&[(VolumeType::Perimiter, ColourType::TiledSpectrum), (VolumeType::Plane, ColourType::RedZGradient), (VolumeType::Pillar, ColourType::ClearBlue)]
			}
		}	
	}
}
impl VolumeType {
	pub fn new(&self, pos: Vec3, max_level_size: f32) -> Vec4 {
		match self {
			VolumeType::Perimiter => {
				let mut dist = f32::abs(pos.x);
				let mut norm = Vec3::new(1.0, 0.0, 0.0);
				if pos.x < 0.0 {
					norm = Vec3::new(1.0, 0.0, 0.0);
				}
				if f32::abs(pos.y) > dist {
					dist = f32::abs(pos.y);
					norm = Vec3::new(0.0, 1.0, 0.0);
					if pos.x < 0.0 {
						norm = Vec3::new(0.0, 1.0, 0.0);
					}
				}
				if f32::abs(pos.z) > dist {
					dist = f32::abs(pos.z);
					norm = Vec3::new(0.0, 0.0, 1.0);
					if pos.x < 0.0 {
						norm = Vec3::new(0.0, 0.0, 1.0);
					}
				}
				Vec4::new(max_level_size - dist - 1.0, norm.x, norm.y, norm.z)
			}, 
			VolumeType::Pillar => {
				let disp = (pos.x + (max_level_size * 0.5), pos.y, pos.z );
				Vec4::new(f32::sqrt((pos.x + (max_level_size * 0.5)).powi(2) + pos.z.powi(2)) - max_level_size * 0.2, disp.0, disp.1, disp.2)
			}, 
			VolumeType::Plane => {
				let plane1 = Vec3::new(1.0, 1.0, 10.0);
				let z = (((pos.x * plane1.x + pos.y * plane1.y) / plane1.z) + max_level_size) - pos.z;
				Vec4::new(z, plane1.x, plane1.y, plane1.z)
			}, 
		}
	} 
}
impl ColourType {
	pub fn new(&self, pos: IVec3, max_level_size: i32) -> Octant {
		match self {
			ColourType::TiledSpectrum => {
				let mut octant = Octant::new();
				let mut tiling_pos= IVec3::new(0, 0, 0);

				tiling_pos.x = if pos.x >= 0 {pos.x} else {max_level_size as i32 + pos.x};
				tiling_pos.y = if pos.y >= 0 {pos.y} else {max_level_size as i32 + pos.y};
				tiling_pos.z = if pos.z >= 0 {pos.z} else {max_level_size as i32 + pos.z};

				let r: u8 = (tiling_pos.x * 255 / max_level_size) as u8;
				let g: u8 = (tiling_pos.y * 255 / max_level_size) as u8;
				let b: u8 = (tiling_pos.z * 255 / max_level_size) as u8;
				let a: u8 = 255;
				octant.colour = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);

				return octant;
			}, 
			ColourType::RedZGradient => {
				let mut octant = Octant::new();

				let r: u8 = (pos.z * 255 / max_level_size) as u8;
				let g: u8 = 50 as u8;
				let b: u8 = 50 as u8;
				let a: u8 = 255;
				octant.colour = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);

				return octant;
			}, 
			ColourType::ClearBlue => {
				let mut octant = Octant::new();
				octant.colour = pack_f32_u32(Vec4::new(0.1, 0.1, 0.5, 0.01));
				return octant;
			}, 
			ColourType::ColouredWalls => {
				let mut octant = Octant::new();
				octant.colour = pack_f32_u32(Vec4::new(1.0, 0.0, 0.0, 1.0));
				let mut dist = i32::abs(pos.x);
				if i32::abs(pos.y) > dist {
					octant.colour = pack_f32_u32(Vec4::new(0.0, 1.0, 0.0, 1.0));
					dist = i32::abs(pos.y);
				}
				if i32::abs(pos.z) > dist {
					octant.colour = pack_f32_u32(Vec4::new(0.0, 0.0, 1.0, 1.0));
				}
				return octant;
			}
		}
	} 
}

/*
positive values are considered to be inside a volume
they do not need to be bound at or below 1
len should be in relation to the level size tho, so long as it uses the pos
 */
fn unpack_f32_u32(data: u32) -> Vec4 {
	let bytes = unpack_u8_u32(data);
	Vec4::new(bytes.0 as f32 / 255.0, bytes.1 as f32 / 255.0, bytes.2 as f32 / 255.0, bytes.3 as f32 / 255.0)
}
//assumed that the floats passed in are < 1
fn pack_f32_u32(data: Vec4) -> u32 {
	pack_u8_u32(((data.x * 255.0) as u8, (data.y * 255.0) as u8, (data.z * 255.0) as u8, (data.w * 255.0) as u8))
}
fn unpack_u8_u32(data: u32) -> (u8, u8, u8, u8) {
	((data >> 24 & MASK_8BIT) as u8,
	(data >> 16 & MASK_8BIT) as u8,
	(data >> 8 & MASK_8BIT) as u8,
	(data & MASK_8BIT) as u8)
}
fn pack_u8_u32(data: (u8, u8, u8, u8)) -> u32 {
	((data.0 as u32) << 24) |
	((data.1 as u32) << 16) |
	((data.2 as u32) << 8) |
	(data.3 as u32)
}

impl Node {
	pub fn new() -> Node {
		return Node{octants: [Octant::new(); 8]};
	}
}

impl Octant {
	pub fn new() -> Octant {
		return Octant{index: NULL_INDEX, colour: 0, normal: 0, extra: 0};
	}
}
