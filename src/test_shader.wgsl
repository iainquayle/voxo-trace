
struct Node {
	info: array<vec3<i32>, 8>;
};

struct Test {
	test: vec4<i32>;
};

[[group(0), binding(0)]] var<storage> buffer : Test;//Node;
[[group(0), binding(1)]] var output: texture_storage_2d<rgba8unorm, write>;

[[stage(compute), workgroup_size(16, 16)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {

	let coords = vec2<i32>(global_id.xy);
	let fCoords = vec2<f32>(coords);
	let dims = textureDimensions(output);
	let fDims = vec2<f32>(dims);

	textureStore(output, coords, vec4<f32>(sin(fCoords.x * fCoords.y / 150.0), sin(fCoords.x * fCoords.y / 100.0), sin(fCoords.x * fCoords.y / 50.0), 1.0));
}