//alias DagAddress = u32; not yet supported by naga?

struct Octant {
	index: u32, //null is u32 max, 0xFFFFFFFF
	colour: u32, //rgba
	normal: u32, //24 bits normal, 8 for density 
	extra: u32, //, 8 for shine,  16 for frames // 8 would be for indexing in larger tree 
}
struct Node { //vec3 ints, x = index, y = colour, z = addition info
	octants: array<Octant, 8>,
}
struct Dag {
	nodes: array<Node>,
}

struct StackEntry {
	index: u32,
	center: vec3<f32>,
}

struct ViewInput {
	position: vec4<f32>, //x y z pad
	radians: vec4<f32>, //yaw, pitch, roll, fov 
}
struct LightInput {
	position: vec4<f32>, //x y z pad
	radians: vec4<f32>, //yaw, pitch, roll, fov 
}

struct ViewData {
	position: vec3<f32>,
	len: f32,
	rgba: u32,
	normal: u32,	
}
struct LightData {
	rgb_len: vec3<f32>,
	rgb: vec3<f32>,
}

const MAX_DEPTH: i32 = 16;
const NULL_INDEX: u32 = 0xFFFFFFFFu;
const MASK_8BIT: u32 = 0x000000FFu;
const MAX_SIZE: f32 = 32768.0;
const MAX_ITERS: u32 = 256u;
const MIN_TRANS: f32 = 0.001;
const FOV: f32 = 1.1;

const POSITIVE_X: u32 = 1u;
const POSITIVE_Y: u32 = 2u;
const POSITIVE_Z: u32 = 4u;
const NEGATIVE_OCTANT: u32 = 0u;
const POSITIVE_MASKS: vec3<u32> = vec3<u32>(POSITIVE_X, POSITIVE_Y, POSITIVE_Z);

@group(0) @binding(0) var<storage, read> dag: Dag;
@group(0) @binding(100) var<uniform> camera: ViewInput;
@group(0) @binding(200) var<storage, write> view: ViewData;
@group(0) @binding(300) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8u, 4u)
fn view_trace(@builtin(global_invocation_id) global_id: vec3<u32>) {
	let dims = vec2<f32>(textureDimensions(output));
	var lod_factor: f32 = sin(FOV / dims.x);

	var level_size: f32 = MAX_SIZE;
	var depth: i32 = 0;
	//if switch is made to 64bit positioning and indexing, keeping octant number only can be done to calculate the centers
	var stack: array<StackEntry, MAX_DEPTH>;
	stack[depth] = StackEntry(0u, vec3<f32>(0.0));
	var octant_index: u32 = 0u;
	var moving_up: bool = false;

	let direction_vec: vec3<f32> = normalize(rotation(get_view_vec(vec2<f32>(global_id.xy), dims), camera.radians.xyz));
	let inverse_vec: vec3<f32> = vec3<f32>(1.0) / direction_vec;
	//can make center an integer if need be if stack entries become index type
	var center: vec3<f32> = vec3<f32>(0.0);
	var position: vec3<f32> = camera.position.xyz; //rays position

	//once volumetric lighting is included then it will need to be changes so that there is a transmittance rgba, and an actual rgba
	//the distinction being what light will be allowed to come through vs what light has already come through
	var transmittance: vec4<f32> = vec4<f32>(1.0);
	//var transmittance: f32 = 1.0;
	var rgb: vec3<f32> = vec3<f32>(0.0);
	var last_octant: Octant;

	var iters: u32 = 0u;
	var length: f32 = 0.0;
	
	loop { if(depth < 0 || transmittance.w < MIN_TRANS || iters > MAX_ITERS) {break;}
		octant_index = dot(POSITIVE_MASKS,
			vec3<u32>(position > center || ((position == center) && direction_vec > 0.0))); 	

		let octant: Octant = dag.nodes[stack[depth].index].octants[octant_index];
		//let lod_size: f32 = max(length * lod_factor, 1.0 / f32(MAX_ITERS - iters) * MAX_SIZE);
		//let bottom: bool = octant.index == NULL_INDEX || length * lod_factor > level_size || 8.0 / f32(MAX_ITERS + 8u - iters) * MAX_SIZE > level_size;
		let bottom: bool = octant.index == NULL_INDEX 
			|| length * lod_factor > level_size 
			|| pow(f32(iters) / f32(MAX_ITERS), 8.0) * MAX_SIZE > level_size;


		if(!moving_up && !bottom) {
			stack[depth + 1].index = octant.index;
			level_size /= 2.0;

			center += level_size * (-1.0 + 2.0 * vec3<f32>((octant_index & POSITIVE_MASKS) == POSITIVE_MASKS)); 

			depth += 1;
			stack[depth].center = center;
		} else {
			if(bottom) {
				last_octant = octant;
			}

			var next_position: vec3<f32> = vec3<f32>(MAX_SIZE * 2.0);
			{
				let to_zero: vec3<f32> = (center - position) * inverse_vec;
				var plane_is_ahead: vec3<bool> = to_zero > 0.0 && position != center;
				if (plane_is_ahead.x && and_vec2_elements(to_zero.x < to_zero.yz || to_zero.yz <= 0.0)) {
					next_position = vec3<f32>(center.x, position.yz + to_zero.x * direction_vec.yz);
				} else if (plane_is_ahead.y && (to_zero.y < to_zero.z || to_zero.z <= 0.0)) {
					next_position = position + to_zero.y * direction_vec;
					next_position.y = center.y;
				} else if (plane_is_ahead.z && direction_vec.z != 0.0) {
					next_position = vec3<f32>(position.xy + to_zero.z * direction_vec.xy, center.z);
				}

				//slower???
				//may just be slower until more diverse and heavily branching dags are ran
				/*
				if (plane_is_ahead.x) {
					next_position = vec4<f32>(center.x, position.yz + to_zero.x * direction_vec.yz, to_zero.x);
				} if (plane_is_ahead.y && to_zero.y < next_position.w) {
					next_position = vec4<f32>(position + to_zero.y * direction_vec, to_zero.y);
					next_position.y = center.y;
				} if (plane_is_ahead.z && to_zero.z < next_position.w)  {
					next_position = vec4<f32>(position.xy + to_zero.z * direction_vec.xy, center.z, to_zero.z);
				}
				*/
			}	

			moving_up = !(and_vec3_elements(abs(center - next_position.xyz) <= level_size));
	
			//moving up
			//bench moving some of these back into if statement
			depth -= i32(moving_up); 
			level_size *= f32(1u << u32(moving_up));

			//moving forward
			let len = dot(next_position.xyz - position, direction_vec);
			length += (len * f32(!moving_up));

			//if statment required so that NANs do not proliferate in position
			if(moving_up) {
				center = stack[depth].center;
			} else {
				position = next_position.xyz;
			}

			//the density should add colours, while the alpha should subtract(ie only let through its colour, that being said think of water and how it only refracts blue)
			//suppositione there is a colour behind a coloured smoke, the colours should add but if behind glass, the glass wont allow the coluor through
			//the alpha is still kept with the rgba vec so that may create an option
			//should probably create a doubled up system, where one colour represents the current pixel colour, and the other represents the alpha colour that lets colour through?
			//with the double system, one is the pixel rgb, the other is the transmittance rgb
				//still it is tricky to convey the alpha
				//may be achieved by using a threshold, 
				//ie if the transmittance is already at or under, or som function approximating the idea, the transmittance calculated with the alpha, then its effects are minimized 
				//or the other option being that an alpha value is stored with the transmittance and that is used as the thershold?

			var octant_rgba = unpack4x8unorm_local(last_octant.colour); 
			var octant_norm = unpack4x8unorm_local(last_octant.normal);
			octant_rgba.w = (octant_rgba.w / 255.0);
			octant_norm.w = (octant_norm.w / 255.0);
			if((octant_norm.w > 0.0 && octant_rgba.w > 0.0) && !moving_up) {
				//changes the denisty to value dependant on distance covered and density
				//octant_rgba.w = (len / (len + (3000.0 * (1.0 - octant_rgba.w)))) * (1.0 - rgba.w);
				octant_norm.w = (len / (len + (3000.0 
					* ((1.0 - octant_norm.w) + (1.0 - octant_rgba.w))))) 
					* transmittance.w;
				//octant_rgba.xyz = octant_rgba
				transmittance.w = transmittance.w - octant_norm.w;
				
				rgb = rgb + octant_rgba.xyz * vec3<f32>(octant_norm.w);
			}
		}
		iters = iters + 1u;
	}

	var octant_rgba = unpack4x8unorm_local(last_octant.colour); 
	octant_rgba.w = (octant_rgba.w / 255.0);
	if( octant_rgba.w > 0.0) {
		rgb = rgb + octant_rgba.xyz * vec3<f32>(transmittance.w);
	}
	//var octant_rgba = unpack4x8unorm_local(last_octant.colour);
	//var octant_norm = unpack4x8unorm_local(last_octant.normal);
	//octant_rgba.w = (1.0 - rgba.w);
	//if(octant_rgba.w > 0.0) {
//		rgba = rgba + vec4<f32>(octant_rgba.xyz * vec3<f32>(octant_rgba.w), octant_rgba.w);	
//	}

	
	rgb /= vec3<f32>(255.0);	
	textureStore(output, vec2<i32>(global_id.xy), vec4<f32>(rgb, 1.0));
}


//TODO: generating a vector field texture, then just use that, that will require being put into the rust rather 
//vectors generated stretch vertically but not horizontally? should check with square res
fn and_vec2_elements(vec: vec2<bool>) -> bool {
	return vec.x && vec.y; 
}
fn and_vec3_elements(vec: vec3<bool>) -> bool {
	return vec.x && vec.y && vec.z;
}
fn get_view_vec(coords: vec2<f32>, dims: vec2<f32>) -> vec3<f32> {
	let thetas: vec2<f32> = vec2<f32>(-((coords.x - dims.x / 2.0) / dims.x * FOV * 2.0),
		((coords.y - dims.y / 2.0) / dims.x * FOV * 2.0));
	return cross(vec3<f32>(cos(thetas.x), 0.0, sin(thetas.x)),
		vec3<f32>(0.0, cos(thetas.y), sin(thetas.y)));
}
fn rotation(direction_vec: vec3<f32>, radians: vec3<f32>) -> vec3<f32> {
	var new_vec = vec3<f32>(direction_vec.x,
		cos(radians.y) * direction_vec.y + sin(radians.y) * direction_vec.z,
		cos(radians.y) * direction_vec.z - sin(radians.y) * direction_vec.y);
	return vec3<f32>(cos(radians.x) * new_vec.x - sin(radians.x) * new_vec.z,
		new_vec.y,
		cos(radians.x) * new_vec.z + sin(radians.x) * new_vec.x);
}
//TODO: check if the u32 vec is even needed
fn unpack4x8unorm_local(x: u32) -> vec4<f32> {
	return vec4<f32>(vec4<u32>((x >> 24u) & MASK_8BIT,
		(x >> 16u) & MASK_8BIT,
		(x >> 8u) & MASK_8BIT,
		x & MASK_8BIT));
}
