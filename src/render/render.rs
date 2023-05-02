use std::{fs, fs::File, io::Write};
use pollster::FutureExt;
use wgpu::{*, util::{*}};
use glam::{Vec3, Vec4, UVec4};
use crate::{asset::oct_dag::{Node}, logic::logic::Logic, window::Window};


const REPORT_AFTER_FRAMES: u64 = 500;
//const TARGET_FRAMES: u32 = 300;

/*
read only: 0-99
inputs: 100-199
intermediates: 200-299
output: 300
 */
//TODO: change these to enums? when the preprocessor is created
//other option is to use procedural macros to generate constants that arent already defined
//however having them defined in one place is nice
#[derive(Copy, Clone)]
enum ShaderSharedConstants {
	Group,
	Dag,
	ViewTraceInput,
	TemporalInput,
	ViewData,
	OutputTexture,
	WorkGroupWidth,
	WorkGroupHeight,
	LightGridDimension,
}
impl ShaderSharedConstants{
	pub(super) fn get_value(&self) -> u32 {
		match self {
			_ => {*self as u32} 
		}
	} 
}

const GROUP_INDEX: u32 = 0;
const DAG_INDEX: u32 = 0;
const VIEW_TRACE_INPUT_INDEX: u32 = 100;
const TEMPORAL_INPUT_INDEX: u32 = 102;
const VIEW_DATA_INDEX: u32 = 200;
const OUTPUT_TEXTURE_INDEX: u32 = 300;

const WORK_GROUP_WIDTH: u32 = 8;
const WORK_GROUP_HEIGHT: u32 = 8;

const LIGHT_GRID_DIMENSION: usize = 128;

//macro_rules! SHADERS_PATH {() => {"shaders.wgsl"};}
//so far exper runs better. need to double checl non flattened valid mask
//not sure where to put shaders later if they arenet baked in via macro
macro_rules! SHADERS_PATH {() => {"./src/render/exper_shaders.wgsl"};}
macro_rules! VIEW_TRACE_ENTRY {() => {"view_trace"};}

pub struct Render {
	integrals: RenderIntegrals,

	//compute_shader:
	//view_trace_layout: PipelineLayout,
	view_trace_pipeline: ComputePipeline,

	//view trace bind groups ran in parallel for final shading synchronization
	view_trace_bindgroups: [BindGroup; 2],
	//output_view: TextureView,

	//dag_buffer: Buffer,
	//
	view_input_uniform: Buffer,
	output_texture: Texture,

	//camera: ViewInputData,
	frame_counter: u64,
	previous_frame_time: u128,
	frame_times_micros: [f64; REPORT_AFTER_FRAMES as usize],
	ave_frame_time: f64,
	max_frame_time: u128,
	min_frame_time: u128,	
}

impl Render {
	pub fn new(window: &Window, state: &Logic) -> Self {
		let integrals = RenderIntegrals::new(window);

		let dag_byte_array = unsafe{std::slice::from_raw_parts(state.dag.nodes[..].as_ptr() as *const u8,
			std::mem::size_of::<Node>() * state.dag.nodes.len())};
		//must have trait POD on it, however that allows for things like bit fiddling
		//let byte_dag_arr = bytemuck::bytes_of(&state.dag.nodes);
		let dag_buffer = integrals.device.create_buffer_init(&BufferInitDescriptor{
			label: Some("dag buffer"),
			usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
			contents: dag_byte_array,
		});
		let view_input_uniform = integrals.device.create_buffer( &BufferDescriptor {
			label: Some(" buffer"),
			mapped_at_creation: false,
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
			size: std::mem::size_of::<ViewInputData>() as u64,
		});
		let output_texture = integrals.device.create_texture(&TextureDescriptor {
			label: Some("output texture"),
			size: Extent3d {
				width: window.inner_width(),
				height: window.inner_height(), 
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: TextureDimension::D2,
			format: TextureFormat::Rgba8Unorm,
			usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
			view_formats: &[],
		});

		let create_buffer = |size: usize, label: &str| {
			integrals.device.create_buffer( &BufferDescriptor {
				label: Some(label),
				mapped_at_creation: false,
				usage: BufferUsages::STORAGE,
				size: size as u64,
			})
		};

		let light_grid_buffers =  
			[create_buffer(LIGHT_GRID_DIMENSION.pow(3) * std::mem::size_of::<LightVolume>(), "light grid buffer"),
			create_buffer(LIGHT_GRID_DIMENSION.pow(3) * std::mem::size_of::<LightVolume>(), "light grid buffer"),];
		let view_buffers  =  
			[create_buffer((integrals.surface_config.height 
					* integrals.surface_config.width) as usize 
					* std::mem::size_of::<ViewData>(), 
					"view buffer"),
			create_buffer((integrals.surface_config.height 
					* integrals.surface_config.width) as usize 
					* std::mem::size_of::<ViewData>(),
					"view buffer")];

		let shader_module = integrals.device.create_shader_module(ShaderModuleDescriptor{
			label: Some("shader module"),
			source: ShaderSource::Wgsl(shader_preprocessor(SHADERS_PATH!().to_string()).into()),
		});
		let pipeline_layout = {
			let buffer_entry = |binding_index: u32, buffer_type: BufferBindingType| {
				BindGroupLayoutEntry {
					binding: binding_index,
					visibility: ShaderStages::COMPUTE,
					ty: BindingType::Buffer { 
						ty: buffer_type, 
						has_dynamic_offset: false, 
						min_binding_size: None, 
					},
					count: None,
			}};
			integrals.device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("view trace pipline layout"),
			push_constant_ranges: &[],
			bind_group_layouts: &[
				&integrals.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
					label: Some("view trace bind group layout"),
					entries: &[
						buffer_entry(DAG_INDEX, BufferBindingType::Storage { read_only: true }),
						buffer_entry(VIEW_TRACE_INPUT_INDEX, BufferBindingType::Uniform),
						buffer_entry(VIEW_DATA_INDEX, BufferBindingType::Storage { read_only: false }),
						BindGroupLayoutEntry {
							binding: OUTPUT_TEXTURE_INDEX,
							visibility: ShaderStages::COMPUTE,
							ty: BindingType::StorageTexture { 
								access: StorageTextureAccess::WriteOnly, 
								format: TextureFormat::Rgba8Unorm, 
								view_dimension: TextureViewDimension::D2 
							},
							count: None,
						},
					],
				}),
			],
		})};
		let view_trace_pipeline = integrals.device.create_compute_pipeline(&ComputePipelineDescriptor {
			label: Some("view trace pipeline"),
			layout: Some(&pipeline_layout),
			module: &shader_module,
			entry_point: VIEW_TRACE_ENTRY!(), 
		});	

		
		let view_trace_bindgroups = {
			let create_view_trace_bindgroup = |view: &Buffer| {
				//seems binding resource has hidden lifetime, not sure how to access it for lifetime specified function, thus macro
				macro_rules! create_bindgroup_entry {
					($binding:expr, $resource:expr) => {
						BindGroupEntry {
							binding: $binding,
							resource: $resource,
						}
				};}
				integrals.device.create_bind_group(&BindGroupDescriptor {
					label: Some("view trace bindgroup"),
					layout: &view_trace_pipeline.get_bind_group_layout(GROUP_INDEX),
					entries: &[
						create_bindgroup_entry!(DAG_INDEX, dag_buffer.as_entire_binding()),
						create_bindgroup_entry!(VIEW_TRACE_INPUT_INDEX, view_input_uniform.as_entire_binding()),
						create_bindgroup_entry!(VIEW_DATA_INDEX, view.as_entire_binding()),
						create_bindgroup_entry!(OUTPUT_TEXTURE_INDEX, BindingResource::TextureView(&output_texture.create_view(&wgpu::TextureViewDescriptor::default()))),
					],
				})
			};
			[create_view_trace_bindgroup(&view_buffers[0]), create_view_trace_bindgroup(&view_buffers[1])]
		};


		return Self {
         integrals: integrals,

			view_trace_pipeline: view_trace_pipeline,
			view_trace_bindgroups: view_trace_bindgroups,
			//output_view: output_view,

			//dag_buffer: dag_buffer,
			output_texture: output_texture,
			view_input_uniform: view_input_uniform,

			//camera: camera,
			frame_counter: 0,
			previous_frame_time: 0,
			
			frame_times_micros: [0.0; REPORT_AFTER_FRAMES as usize],
			ave_frame_time: 0.0,
			max_frame_time: 0,
			min_frame_time: 0,
		}
	}


	/**
	 * possible way to render different options, in extra, have a number of bits dedicated to how many possible variations there are
	 * then add the bits to the index so long as the nodes are packed next to each other
	 * shouldnt be an in terms of performance, and it wont take up much in the way of memroy, it also shouldnt be hard to pack them next to each other so long as the build tool is made that way from the start
	 */
	/*
	 *consider changing to not returning a result
	 *take away the question mark below and just us an expect
	 */
	pub fn render(&mut self, state: &Logic) -> Result<(), SurfaceError> {
			self.update_camera(ViewInputData { 
					pos: Into::<Vec4>::into((state.camera_pose().position, 0.0)),
					rads: Into::<Vec4>::into((state.camera_orientaion_vec3(), 0.0)),
			});

		let mut encoder = self.integrals.device.create_command_encoder(&CommandEncoderDescriptor{label: Some("view trace render pass encoder")});
		
		let mut view_trace_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("view trace pass")});
		view_trace_pass.set_pipeline(&self.view_trace_pipeline);
		view_trace_pass.set_bind_group(GROUP_INDEX, &self.view_trace_bindgroups[(self.frame_counter % 2) as usize], &[]);
		view_trace_pass.dispatch_workgroups(self.integrals.surface_config.width / WORK_GROUP_WIDTH, self.integrals.surface_config.height / WORK_GROUP_HEIGHT, 1);

		drop(view_trace_pass);


		let surface_texture = self.integrals.surface.get_current_texture()?;
		encoder.copy_texture_to_texture(
			ImageCopyTexture {
				aspect: TextureAspect::All,
				texture: &self.output_texture,
				mip_level: 0,
				origin: Origin3d::ZERO,
			},
			ImageCopyTexture {
				aspect: TextureAspect::All,
				texture: &surface_texture.texture,
				mip_level: 0,
				origin: Origin3d::ZERO,
			},
			Extent3d {
				width: self.integrals.surface_config.width,
				height: self.integrals.surface_config.height,
				depth_or_array_layers: 1,
			},
		);

		/*
		if state.start_time.elapsed().as_micros() - self.previous_frame_time > 0 {
			thread::sleep(std::time::Duration::from_micros(0));	
		}
		*/

		//if wnating to make a multi stage rendering process, create second encoder, and submitt both at once
		//the second will need to operate on data given by the previous iteration rather than the same
		self.previous_frame_time = state.start_time.elapsed().as_micros();
		self.integrals.queue.submit(std::iter::once(encoder.finish()));
		surface_texture.present();
		let frame_time = state.start_time.elapsed().as_micros() - self.previous_frame_time;
		
		self.frame_times_micros[(self.frame_counter % REPORT_AFTER_FRAMES) as usize] = frame_time as f64 / 1000.0;
		self.ave_frame_time = self.ave_frame_time + frame_time as f64 * 1.0 / REPORT_AFTER_FRAMES as f64;
		if self.max_frame_time < frame_time {
			self.max_frame_time = frame_time;
		}
		if self.min_frame_time > frame_time {
			self.min_frame_time = frame_time;
		}
		if self.frame_counter % REPORT_AFTER_FRAMES  == 0 {
			//println!("fps: {}", 500.0 / ((state.start_time.elapsed().as_millis() - self.fps_prev_time) as f32 / 1000.0));
			println!("frame time (milis): {:.2}, max: {:.2}, min: {:.2}\nideal ave fps: {:.2}, min: {:.2}, max: {:.2}\n--------------------------------------------------------------",
				self.ave_frame_time / 1000.0, self.max_frame_time as f64 / 1000.0, self.min_frame_time as f64 / 1000.0,
				(1000000.0 / self.ave_frame_time), (1000000.0 / self.max_frame_time as f64), (1000000.0 / self.min_frame_time as f64));
			if self.max_frame_time / 1000 > 10 {
				println!("frames recorded");
				let mut file = File::create("./times.csv").expect("unable to make file for frame times");
				for time in self.frame_times_micros {
					file.write_fmt(format_args!("{:.2},\n", time)).expect("failed to write to file frame times");
				}
			}
			self.min_frame_time = u128::MAX;
			self.max_frame_time = 0;
			self.ave_frame_time = 0.0;
		}
		self.frame_counter += 1;

		return Ok(());
	}

	pub fn update_dag(&mut self) {}

	pub fn update_camera(&mut self, pos: ViewInputData) {
		self.integrals.queue.write_buffer(&self.view_input_uniform, 0, unsafe{ std::slice::from_raw_parts((&pos as *const ViewInputData) as *const u8, std::mem::size_of::<ViewInputData>()) });
	}


	pub fn print_state(&self) {
		println!("Device: {}", self.integrals.adapter.get_info().name);
		println!("Backend: {}", match self.integrals.adapter.get_info().backend{
			Backend::BrowserWebGpu => "Browser",
			Backend::Dx12 => "DX12",
			Backend::Vulkan => "Vulkan",
			Backend::Metal => "Metal",
			Backend::Dx11 => "DX11",
			Backend::Gl => "OpenGL",
			Backend::Empty => "None"
		});
	}
}

struct RenderIntegrals {
	pub surface: Surface,
	pub adapter: Adapter,
	pub device: Device,
	pub queue: Queue,
	pub surface_config: SurfaceConfiguration,
} 
impl RenderIntegrals {
	pub fn new(window: &Window) -> Self {
		let instance = Instance::new(InstanceDescriptor{
			backends: Backends::DX12,
			dx12_shader_compiler: Default::default(),	
			//dx12_shader_compiler: wgpu::Dx12Compiler::Dxc { 
			//	dxil_path: Some(PathBuf::from("dxil.dll")),
			//	dxc_path: Some(PathBuf::from("dxcompiler.dll")) 
			//}, 
		});
		let surface = unsafe{instance.create_surface(window.borrow_window())}.expect("failed to create surface");

		let adapter = instance.request_adapter(&RequestAdapterOptions{
			power_preference: PowerPreference::HighPerformance,
			compatible_surface: Some(&surface),
			force_fallback_adapter: false,
		}).block_on().expect("Fail to find suitable adapter");
		
		let limits = adapter.limits();
		let (device, queue) = adapter.request_device(&DeviceDescriptor{
			features: Features::default(),
			//limits: Limits{max_compute_workgroup_storage_size: 24000, ..Default::default()},
			limits: Limits{..Default::default()},
			label: Some("device"),
		}, None).block_on().expect("failed to create device and queue");

		println!("max workgroup storage: {}\ndefault limit: {}",
			limits.max_compute_workgroup_storage_size,
			Limits::default().max_compute_workgroup_storage_size);
		
		//let surface_capabilities = surface.get_capabilities(&adapter);
		//let surface_format = surface_capabilities.formats.iter().copied()
		//	.filter(|f| f.describe().srgb)
		//	.next().unwrap_or(surface_capabilities.formats[0]);
		let surface_config = SurfaceConfiguration {
			usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_DST,
			format: TextureFormat::Rgba8Unorm,//surface.get_preferred_format(&adapter).unwrap()
			width: window.inner_width(), 
			height: window.inner_height(), 
			present_mode: PresentMode::Fifo,
			alpha_mode: CompositeAlphaMode::Opaque,
			view_formats: vec![],
		};
		surface.configure(&device, &surface_config);
        
		Self {
			surface: surface,
			adapter: adapter,
			device: device,
			queue: queue,
			surface_config: surface_config,
		}
	} 
}

//TODO: make pre compiler
fn shader_preprocessor(path: String) -> String {
	match fs::read_to_string(path) {
		Ok(source) => {
			source
		},
		Err(err) => {
			println!("failed to read shader file: {}", err);
			panic!();
		}
	}
}

#[repr(C)]
pub struct _TemporalInputData {
	pub temporals: UVec4, //time, frame, time delta, frame delta
}
//#[repr(C, align(8))]
#[repr(C)]
pub struct ViewInputData {
	pub pos: Vec4, //x, y, z, pad
	pub rads: Vec4, //yaw, pitch, roll, pad(change to fov)
}
#[repr(C)]
pub struct _LightInputData {
	pub pos: Vec4,//x y z pad
	pub dir: Vec4,//x, y, z, fov 
	pub rgb: Vec4,//r g b pad //a could be used to increase itensity?
}

/*
 there structs are required, 
 used to size allocation of data in the device
 */
#[repr(C)]
struct ViewData {
	pos: Vec3,
	len: f32,
	rgba: u32, 		
	normal: u32, 
}
//find some form of direction that does not reqeat ie (1, 1), (2, 2)
#[repr(C)]
struct LightData {
	rgb: f32, //rgb, parellax?
	direction: u32, //xyz, dist?
}
#[repr(C)]
struct LightVolume {
	data: [LightData; 4],
}