type DagIndex = u32; 

//TODO: change to using a vec4, may need to define alignment, but would likely help alot with read speeds
//perhaps create functions for grabbing info to help with readability
//type Octant = vec4<u32>;
struct Octant {
	index: DagIndex, //null is u32 max, 0xFFFFFFFF
	colour: u32, //rgba
	normal: u32, //24 bits normal, 8 for density //may want to change such that density is alpha, that more closely resmbles how the colours add
	extra: u32, //, 8 for shine,  16 for frames // 8 would be for indexing in larger tree 
}
struct Node { //vec3 ints, x = index, y = colour, z = addition info
	octants: array<Octant, 8>,
}
struct Dag {
	nodes: array<Node>,
}

struct ViewInput {
	position: vec4<f32>, //x y z pad
	direction: vec4<f32>, //yaw, pitch, roll, fov 
}

struct ViewData {
	position: vec3<f32>,
	len: f32,
	rgba: u32,
	normal: u32,	
}

const MAX_DEPTH: i32 = 16;
const NULL_INDEX: DagIndex = 0xFFFFFFFFu;
const MASK_8BIT: u32 = 0x000000FFu;
const MAX_SIZE: i32 = 0x008000; //can go up to 20 bits before excessive precission loss
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

@compute @workgroup_size(8u, 8u)
fn view_trace(@builtin(global_invocation_id) global_id: vec3<u32>) {
	let dims = vec2<f32>(textureDimensions(output));
	var lod_factor: f32 = sin(FOV / dims.x);

	var level_size: i32 = MAX_SIZE;
	var depth: i32 = 0;
	var stack: array<DagIndex, MAX_DEPTH>;
	stack[depth] = 0u; 
	var octant_index: u32 = 0u;
	var moving_up: bool = false;

	var direction: vec3<f32> = normalize(rotation(get_view_vec(vec2<f32>(global_id.xy), dims), camera.direction.xyz));
	var inverse_vec: vec3<f32> = vec3<f32>(1.0) / direction;
	var i_center: vec3<i32> = vec3<i32>(MAX_SIZE);
	var center: vec3<f32> = vec3<f32>(0.0);
	var position: vec3<f32> = camera.position.xyz; 
		
	var transmittance: vec4<f32> = vec4<f32>(1.0);
	var rgb: vec3<f32> = vec3<f32>(0.0);
	var previous_octant: Octant;

	var iters: u32 = 0u;
	var length: f32 = 0.0;
	
	loop { if(depth < 0 || transmittance.w < MIN_TRANS || iters > MAX_ITERS) {break;}
		octant_index = dot(POSITIVE_MASKS,
			vec3<u32>(position > center || ((position == center) && direction > 0.0))); 	

		let octant: Octant = dag.nodes[stack[depth]].octants[octant_index];
		let bottom: bool = octant.index == NULL_INDEX 
			|| length * lod_factor > f32(level_size)
			|| pow(f32(iters) / f32(MAX_ITERS), 8.0) * f32(MAX_SIZE) > f32(level_size);

		if(!moving_up && !bottom) {
			stack[depth + 1] = octant.index;
			depth += 1;
			level_size >>= 1u;
			i_center += level_size * (-1 + 2 * vec3<i32>((octant_index & POSITIVE_MASKS) == POSITIVE_MASKS)); 
			center = vec3<f32>(i_center - MAX_SIZE);
		} else {
			if(bottom) {
				previous_octant = octant;
			}

			let to_zero: vec3<f32> = (center - position) * inverse_vec;
			var valid: vec3<bool> = to_zero > 0.0 && position != center;
			valid &= (to_zero <= to_zero.zxy || !valid.zxy) && (to_zero < to_zero.yzx || !valid.yzx);
			//let next_position = select(position + direction * dot(vec3<f32>(valid), to_zero), center, valid); 
			//for some reason this runs more stable than the select
			//bench mark putting valid back into the following if block
			//while the averages seem fine, the lows are very volatile
			var next_position = vec3<f32>(0.0);
			if(valid.x) {
				next_position = vec3<f32>(center.x, position.yz + direction.yz * to_zero.x);
			} else if(valid.y) {
				next_position = position + direction * to_zero.y;
				next_position.y = center.y;
			} else if(valid.z) {
				next_position = vec3<f32>(position.xy + direction.xy * to_zero.z, center.z);
			}

			//can also use position == next instead of valid
			moving_up = any(abs(center - next_position) > f32(level_size)) || all(!valid);
	
			//moving up
			i_center += (level_size * (-1 + 2 * vec3<i32>((i_center - level_size) % (level_size * 4) == 0))) * i32(moving_up); 
			center = vec3<f32>(i_center - MAX_SIZE);
			depth -= i32(moving_up); 
			level_size <<= u32(moving_up);
			
			//moving forward
			let len = dot(next_position.xyz - position, direction);
			length += (len * f32(!moving_up));
			if(!moving_up) {
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

			var octant_rgba = unpack4x8unorm_local(previous_octant.colour) / 255.0; 
			var octant_norm = unpack4x8unorm_local(previous_octant.normal) / 255.0;
			if((octant_norm.w > 0.0 && octant_rgba.w > 0.0) && !moving_up) {
				octant_norm.w = (len / (len + (3000.0 
					* ((1.0 - octant_norm.w) + (1.0 - octant_rgba.w))))) 
					* transmittance.w;
				transmittance.w = transmittance.w - octant_norm.w;
				rgb = rgb + octant_rgba.xyz * vec3<f32>(octant_norm.w);
			}
		}
		iters += 1u;
	}

	var octant_rgba = unpack4x8unorm_local(previous_octant.colour) / 255.0; 
	if( octant_rgba.w > 0.0) {
		rgb = rgb + octant_rgba.xyz * vec3<f32>(transmittance.w);
	}
	textureStore(output, vec2<i32>(global_id.xy), vec4<f32>(rgb, 1.0));
}

//vectors generated stretch vertically but not horizontally? should check with square res
fn get_view_vec(coords: vec2<f32>, dims: vec2<f32>) -> vec3<f32> {
	let thetas: vec2<f32> = vec2<f32>(-((coords.x - dims.x / 2.0) / dims.x * FOV * 2.0),
		((coords.y - dims.y / 2.0) / dims.x * FOV * 2.0));
	return cross(vec3<f32>(cos(thetas.x), 0.0, sin(thetas.x)),
		vec3<f32>(0.0, cos(thetas.y), sin(thetas.y)));
}
fn rotation(direction: vec3<f32>, radians: vec3<f32>) -> vec3<f32> {
	var new_vec = vec3<f32>(direction.x,
		cos(radians.y) * direction.y + sin(radians.y) * direction.z,
		cos(radians.y) * direction.z - sin(radians.y) * direction.y);
	return vec3<f32>(cos(radians.x) * new_vec.x - sin(radians.x) * new_vec.z,
		new_vec.y,
		cos(radians.x) * new_vec.z + sin(radians.x) * new_vec.x);
}
fn unpack4x8unorm_local(x: u32) -> vec4<f32> {
	return vec4<f32>(vec4<u32>((x >> 24u) & MASK_8BIT,
		(x >> 16u) & MASK_8BIT,
		(x >> 8u) & MASK_8BIT,
		x & MASK_8BIT));
}
