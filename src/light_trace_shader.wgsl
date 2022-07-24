struct Octant {
	index: u32; //null is u32 max, 0xFFFFFFFF
	colour: u32; //rgba
	normal: u32; //24 bits normal, 8 for density 
	extra: u32; //, 8 for shine,  16 for frames // 8 would be reserved for a larger tree but as it is, memory constraints make it not feasible anyways
};
struct Node { //vec3 ints, x = index, y = colour, z = addition info
	octants: array<Octant, 8>;
};
struct NodeBuffer {
	nodes: array<Node>;
};

struct StackEntry {
	index: u32;
	center: vec3<f32>;
};

struct ViewInput {
	pos: vec4<f32>; //x y z pad
	rads: vec4<f32>; //yaw, pitch, roll, pad
	light_pos: vec4<f32>;
	temporals: vec4<u32>; //time, frames
};
struct LightInput {
	pos: vec4<f32>; //x y z pad
	rads: vec4<f32>; //yaw, pitch, roll, pad
	light_pos: vec4<f32>;
	temporals: vec4<u32>; //time, frames
};

struct ViewData {
	pos: vec3<f32>;
	len: f32;
	rgba: u32;
	normal: u32;	
};
struct LightData {
	rgb_len: vec3<f32>;
	rgb: vec3<f32>;
};

let MAX_DEPTH: i32 = 16;
let NULL_INDEX: u32 = 0xFFFFFFFFu;
let MASK_8BIT: u32 = 0x000000FFu;
let MAX_SIZE: f32 = 32768.0;
let MAX_ITERS: u32 = 256u;
let MAX_ALPHA: f32 = 0.999;
let FOV: f32 = 1.1;

let POSITIVE_X: u32 = 1u;
let POSITIVE_Y: u32 = 2u;
let POSITIVE_Z: u32 = 4u;
let NEGATIVE_OCTANT: u32 = 0u;

[[group(0), binding(0)]] var<storage, read> dag: NodeBuffer;
[[group(0), binding(100)]] var<uniform> camera: ViewInput;
[[group(0), binding(200)]] var<storage, write> view: ViewData;
[[group(0), binding(300)]] var output: texture_storage_2d<rgba8unorm, write>;

//TODO: generating a vector field texture, then just use that, that will require being put into the rust rather than here
//vectors generated stretch vertically but not horizontally? should check with square res
fn get_view_vec(coords: vec2<f32>, dims: vec2<f32>) -> vec3<f32> {
	let thetas: vec2<f32> = vec2<f32>(-((coords.x - dims.x / 2.0) / dims.x * FOV * 2.0), ((coords.y - dims.y / 2.0) / dims.x * FOV * 2.0));
	return cross(vec3<f32>(cos(thetas.x), 0.0, sin(thetas.x)), vec3<f32>(0.0, cos(thetas.y), sin(thetas.y)));
}
fn rot(dir_vec: vec3<f32>, rads: vec3<f32>) -> vec3<f32> {
	var new_vec = vec3<f32>(dir_vec.x, cos(rads.y) * dir_vec.y + sin(rads.y) * dir_vec.z, cos(rads.y) * dir_vec.z - sin(rads.y) * dir_vec.y);
	return vec3<f32>(cos(rads.x) * new_vec.x - sin(rads.x) * new_vec.z, new_vec.y, cos(rads.x) * new_vec.z + sin(rads.x) * new_vec.x);
}
fn unpack4x8unorm_local(x: u32) -> vec4<f32> {
	return vec4<f32>(vec4<u32>(x >> 24u & MASK_8BIT, x >> 16u & MASK_8BIT, x >> 8u & MASK_8BIT, x & MASK_8BIT));
}
//this shouldn't be used to the issue of float inaccuracy at larger numbers?
//even with the 0.5 rounding, suppose that a float cant match the top end accuracy of the u32?
//when tested in rust there seemed to be no loss, that is assuming it would work on the gpu as well, and on all implementations
fn int_index(index: f32) -> u32 {
	return u32(index + 0.5);
}

[[stage(compute), workgroup_size(8, 8)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
	// TODO: consider changing index in stack entry to an f32 in the w position and casting whenever required
	// this shouldnt be a problem as any time that a null is encountered, it doesnt need to be entered in the stack or used again

	//automate this!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
	let dims = vec2<f32>(textureDimensions(output));
	var lod_factor: f32 = sin(FOV / dims.x);

	var level_size: f32 = MAX_SIZE;
	var stack: array<StackEntry, MAX_DEPTH>;
	var depth: i32 = 0;
	//to create system for multi level colouring
	//make it so that the stack entries hold the colour of the octant in the stack entry data
	//and the length at which the octant was entered
	//struct {index and octant data vec2<u32>, center and length vec4<f32>}
	//shouldnt take more room than current implementation due to the required padding, but will if switched to the float indexing
	//when tested with rust it came out as the same thing tho...
	stack[depth] = StackEntry(0u, vec3<f32>(0.0));
	var octant_index: u32 = 0u;
	var moving_up: bool = false;

	let dir_vec: vec3<f32> = normalize(rot(get_view_vec(vec2<f32>(global_id.xy), dims), camera.rads.xyz));//vecs can be put in the push constants and not tyouched again, will migrate these out later
	let inv_vec: vec3<f32> = vec3<f32>(1.0) / dir_vec;
	var center: vec3<f32> = vec3<f32>(0.0);
	var pos: vec3<f32> = camera.pos.xyz;//vec3<f32>(-10000.0, 12000.0, 0.0); //change to starting on a push contant

	var rgba: vec4<f32> = vec4<f32>(0.0);
	var last_octant: Octant;

	var iters: u32 = 0u;
	var length: f32 = 0.0;
	
	//view trace
	//no lighting accomplished
	loop { if(depth < 0 || rgba.w > MAX_ALPHA || iters > MAX_ITERS) {break;}
		//if(pos.x > center.x || ((pos.x == center.x) && dir_vec.x > 0.0)) { octant_index = POSITIVE_X; } else { octant_index = NEGATIVE_OCTANT; }
		//if(pos.y > center.y || ((pos.y == center.y) && dir_vec.y > 0.0)) { octant_index = octant_index + POSITIVE_Y; }
		//if(pos.z > center.z || ((pos.z == center.z) && dir_vec.z > 0.0)) { octant_index = octant_index + POSITIVE_Z; }

		octant_index = POSITIVE_X * u32(pos.x > center.x || ((pos.x == center.x) && dir_vec.x > 0.0));
		octant_index = octant_index + POSITIVE_Y * u32(pos.y > center.y || ((pos.y == center.y) && dir_vec.y > 0.0));
		octant_index = octant_index + POSITIVE_Z * u32(pos.z > center.z || ((pos.z == center.z) && dir_vec.z > 0.0));
		

		let octant: Octant = dag.nodes[stack[depth].index].octants[octant_index];
		//let lod_size: f32 = max(length * lod_factor, 1.0 / f32(MAX_ITERS - iters) * MAX_SIZE);
		let bottom: bool = octant.index == NULL_INDEX || length * lod_factor > level_size || 1.0 / f32(MAX_ITERS - iters) * MAX_SIZE > level_size;
		//let bottom: bool = octant.index == NULL_INDEX || length * lod_factor > level_size || pow(f32(iters) / f32(MAX_ITERS), 5.0) * MAX_SIZE > level_size;
		if(!moving_up && !bottom) {
			stack[depth + 1].index = octant.index;
			level_size = level_size / 2.0;

			//center = center + level_size * (2.0 * f32((octant_index & POSITIVE_X) == POSITIVE_X) - 1.0);
			if((octant_index & POSITIVE_X) == POSITIVE_X) { center.x = center.x + level_size; }
			else { center.x = center.x - level_size; }
			if((octant_index & POSITIVE_Y) == POSITIVE_Y) { center.y = center.y + level_size; }
			else { center.y = center.y - level_size; }
			if((octant_index & POSITIVE_Z) == POSITIVE_Z) { center.z = center.z + level_size; }
			else { center.z = center.z - level_size; }

			//center.x = center.x + level_size * f32(1 | (i32((octant_index & POSITIVE_X) == POSITIVE_X) << 31));
			//center.y = center.y + level_size * f32(1 | (i32((octant_index & POSITIVE_Y) == POSITIVE_Y) << 31));
			//center.z = center.z + level_size * f32(1 | (i32((octant_index & POSITIVE_Z) == POSITIVE_Z) << 31));

			depth = depth + 1;
			stack[depth].center = center;
		} else {
			if(bottom) {
				last_octant = octant;
			}

			//var colour_factor = 0.0;
			//if(lod_size < level_size && lod_size > level_size / 2.0) {
			//	colour_factor = lod_size / level_size;
			//} else if (lod_size > level_size || octant.index == NULL_INDEX) {
			//	colour_factor = 1.0;
			//}
			//may be some ways in which more performance could be gained by doing more with vector operations
			//ie next pos, abs around the plane checker

			let to_zero: vec3<f32> = (center - pos) * inv_vec;
			var next_pos: vec3<f32> = vec3<f32>(MAX_SIZE * 2.0);

			if ((to_zero.x > 0.0 && pos.x != center.x) && (to_zero.x < to_zero.y || to_zero.y <= 0.0) && (to_zero.x < to_zero.z || to_zero.z <= 0.0)) {
				next_pos = vec3<f32>(center.x, pos.y + to_zero.x * dir_vec.y, pos.z + to_zero.x * dir_vec.z);
			} else if((to_zero.y > 0.0 && pos.y != center.y) && (to_zero.y < to_zero.z || to_zero.z <= 0.0)) {
				next_pos = vec3<f32>(pos.x + to_zero.y * dir_vec.x, center.y, pos.z + to_zero.y * dir_vec.z);
			} else if((to_zero.z > 0.0 && pos.z != center.z && dir_vec.z != 0.0)) {
				next_pos = vec3<f32>(pos.x + to_zero.z * dir_vec.x, pos.y + to_zero.z * dir_vec.y, center.z);
			}

			//var next_pos: vec4<f32> = vec4<f32>(MAX_SIZE * 2.0);
			//var is_next: bool = to_zero.x > 0.0 && pos.x != center.x;
			//next_pos = next_pos * vec4<f32>(f32(!is_next)) + vec4<f32>(center.x, pos.y + to_zero.x * dir_vec.y, pos.z + to_zero.x * dir_vec.z, to_zero.x) * vec4<f32>(f32(is_next));
			//is_next = to_zero.y > 0.0 && to_zero.y < next_pos.w && pos.y != center.y;
			//next_pos = next_pos * vec4<f32>(f32(!is_next)) + vec4<f32>(pos.x + to_zero.y * dir_vec.x, center.y, pos.z + to_zero.y * dir_vec.z, to_zero.y) * vec4<f32>(f32(is_next));
			//is_next = to_zero.z > 0.0 && to_zero.z < next_pos.w && pos.z != center.z;
			//next_pos = next_pos * vec4<f32>(f32(!is_next)) + vec4<f32>(pos.x + to_zero.z * dir_vec.x, pos.y + to_zero.z * dir_vec.y, center.z, to_zero.z) * vec4<f32>(f32(is_next));
			
			

			//the flattened moving clauses may not actually be the best solutions
			//may want something that is a hybrid of the two, where stements that are easy to make in a such a way are flattened while other not
			moving_up = !(abs(center.x - next_pos.x) <= level_size && abs(center.y - next_pos.y) <= level_size && abs(center.z - next_pos.z) <= level_size);		
			//moving forward
			let len = dot(next_pos - pos, dir_vec);
			length = length + (len * f32(!moving_up));
			pos = pos * vec3<f32>(f32(moving_up)) + next_pos * vec3<f32>(f32(!moving_up));

			//moving up
			depth = depth - i32(moving_up); 
			center = center * vec3<f32>(f32(!moving_up)) + stack[depth].center * vec3<f32>(f32(moving_up));
			level_size = level_size * f32(1u << u32(moving_up));//level_size * f32(1u << u32(moving_up));

			//if(moving_up) {
			//	center = stack[depth].center;
			//} else {
			//	pos = pos + next_pos;
			//}

			var octant_rgba = unpack4x8unorm_local(last_octant.colour);
			var octant_norm = unpack4x8unorm_local(last_octant.normal);
			octant_rgba.w = (octant_rgba.w / 255.0) * (1.0 - rgba.w);
			if(octant_rgba.w > 0.0 && !moving_up) {
				octant_rgba.w = (len / (len + (2000.0 * (1.0 - octant_rgba.w)))) * (1.0 - rgba.w);
				rgba = rgba + vec4<f32>(octant_rgba.xyz * vec3<f32>(octant_rgba.w), octant_rgba.w);
			}
		
			//if(!moving_up) {
				//let len = dot(next_pos - pos, dir_vec);
				//var octant_rgba = unpack4x8unorm_local(last_octant.colour);
				//octant_rgba.w = (octant_rgba.w / 255.0) * (1.0 - rgba.w);
				//if(octant_rgba.w > 0.0) {
					//octant_rgba.w = (len / (len + (2000.0 * (1.0 - octant_rgba.w)))) * (1.0 - rgba.w);
					//rgba = rgba + vec4<f32>(octant_rgba.xyz * vec3<f32>(octant_rgba.w), octant_rgba.w);
				//}

				//length = length + len;
				//pos = next_pos;
			//} else {
				//depth = depth - 1;
				//center = stack[depth].center;
				//level_size = level_size * 2.0;
			//}
		
		}
		iters = iters + 1u;
	}


	var octant_rgba = unpack4x8unorm_local(last_octant.colour);
	var octant_norm = unpack4x8unorm_local(last_octant.normal);
	octant_rgba.w = (1.0 - rgba.w);
	if(octant_rgba.w > 0.0) {
		rgba = rgba + vec4<f32>(octant_rgba.xyz * vec3<f32>(octant_rgba.w), octant_rgba.w);	
	}

	
	rgba = rgba / vec4<f32>(255.0);	

	if(iters >= MAX_ITERS - 1u) {
		textureStore(output, vec2<i32>(global_id.xy), vec4<f32>(1.0, 0.0, 0.0, 1.0));
	} else {
		textureStore(output, vec2<i32>(global_id.xy), rgba);
	}
}

