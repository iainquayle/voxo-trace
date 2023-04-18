use std::{fs::File, io::Write};
use pollster::FutureExt;
use wgpu::{include_wgsl, util::{DeviceExt, BufferInitDescriptor}, Surface, Adapter, Device, Queue, 
	SurfaceConfiguration, BufferDescriptor, BufferUsages, Instance, Backends, PowerPreference, Features, Limits,
	TextureUsages, TextureFormat, PresentMode, CompositeAlphaMode, DeviceDescriptor, RequestAdapterOptions,
	InstanceDescriptor, ComputePipeline, BindGroup, Texture, Buffer, TextureDescriptor, Extent3d, 
	TextureDimension, PipelineLayoutDescriptor, BindGroupLayoutDescriptor, BindGroupLayoutEntry, 
	ShaderStages, BindingType, BufferBindingType, Backend, StorageTextureAccess, TextureViewDimension, 
	ComputePipelineDescriptor, BindGroupDescriptor, BindGroupEntry, BindingResource, SurfaceError, 
	CommandEncoderDescriptor, ComputePassDescriptor, ImageCopyTexture, TextureAspect, Origin3d};
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
const GROUP_INDEX: u32 = 0;
const DAG_INDEX: u32 = 0;
const VIEW_TRACE_INPUT_INDEX: u32 = 100;
const TEMPORAL_INPUT_INDEX: u32 = 102;
const VIEW_DATA_INDEX: u32 = 200;
const OUTPUT_TEXTURE_INDEX: u32 = 300;

const WORK_GROUP_WIDTH: u32 = 8;
const WORK_GROUP_HEIGHT: u32 = 4;

const LIGHT_TEXTURE_WIDTH: u32 = 1024;
const LIGHT_TEXTURE_HEIGHT: u32 = 1024;

macro_rules! SHADERS_PATH {() => {"shaders.wgsl"};}
//macro_rules! SHADERS_PATH {() => {"exper_shaders.wgsl"};}
macro_rules! VIEW_TRACE_ENTRY {() => {"view_trace"};}



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
	pub _pos: Vec3,
	pub _len: f32,
	pub _rgba: u32, 		
	pub _normal: u32, 
}
#[repr(C)]
struct LightData {
	pub _rgb_len: Vec3,
	pub _rgb: Vec3,
}


struct RenderAgnostics {
	pub surface: Surface,
	pub adapter: Adapter,
	pub device: Device,
	pub queue: Queue,
	pub surface_config: SurfaceConfiguration,
} 
pub struct Render {
	agnostics: RenderAgnostics,

	//compute_shader:
	//view_trace_layout: PipelineLayout,
	view_trace_pipeline: ComputePipeline,

	//view trace bind groups ran in parallel for final shading synchronization
	view_trace_bindgroups: [BindGroup; 2],
	//output_view: TextureView,

	//dag_buffer: Buffer,
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
		let agnostics = RenderAgnostics::new(window);

		let byte_dag_array = unsafe{std::slice::from_raw_parts(state.dag.nodes[..].as_ptr() as *const u8,
			std::mem::size_of::<Node>() * state.dag.nodes.len())};
		//must have trait POD on it, however that allows for things like bit fiddling
		//let byte_dag_arr = bytemuck::bytes_of(&state.dag.nodes);
		let dag_buffer = agnostics.device.create_buffer_init(&BufferInitDescriptor{
			label: Some("dag buffer"),
			usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
			contents: byte_dag_array,
		});
		let view_input_uniform = agnostics.device.create_buffer( &BufferDescriptor {
			label: Some(" buffer"),
			mapped_at_creation: false,
			usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
			size: std::mem::size_of::<ViewInputData>() as u64,
		});
		let output_texture = agnostics.device.create_texture(&TextureDescriptor {
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
		//returns buffers required to make one lane in double buffered pipline
		//does not need to create the input uniform or buffers or the output texture
		let (view_buffers, _light_buffers)  = { 
			let create_buffer = |size: usize| {
				agnostics.device.create_buffer( &BufferDescriptor {
					label: Some("view data buffer"),
					mapped_at_creation: false,
					usage: BufferUsages::STORAGE,
					size: size as u64,
				})
			};
			/*
			light buffer when creating multiples will either need to be made as multiple buffers, or one large buffer
			should be ok since the buffer can be treated as a muti dimensionsonal array once it is in the shader
			 */

			([create_buffer((agnostics.surface_config.height 
					* agnostics.surface_config.width 
					* std::mem::size_of::<ViewData>() as u32) as usize),
			create_buffer((agnostics.surface_config.height 
					* agnostics.surface_config.width 
					* std::mem::size_of::<ViewData>() as u32) as usize)], 
			[create_buffer((LIGHT_TEXTURE_WIDTH 
					* LIGHT_TEXTURE_HEIGHT 
					* std::mem::size_of::<LightData>() as u32) as usize),
			create_buffer((LIGHT_TEXTURE_WIDTH 
					* LIGHT_TEXTURE_HEIGHT 
					* std::mem::size_of::<LightData>() as u32) as usize)])
		};
		
		/*
		create view trace pipeline
		TODO: this should be able to be turned into just a singular pipline layout, not differentiated as view trace
		 */
		let view_trace_shader = agnostics.device.create_shader_module(include_wgsl!(SHADERS_PATH!()));
		let view_trace_layout = {
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
			agnostics.device.create_pipeline_layout(&PipelineLayoutDescriptor {
			label: Some("view trace pipline layout"),
			push_constant_ranges: &[],
			bind_group_layouts: &[
				&agnostics.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
		let view_trace_pipeline = agnostics.device.create_compute_pipeline(&ComputePipelineDescriptor {
			label: Some("view trace pipeline"),
			layout: Some(&view_trace_layout),
			module: &view_trace_shader,
			entry_point: VIEW_TRACE_ENTRY!(), 
		});	

		

		/*
		 */
		let create_view_trace_bindgroup = |view: &Buffer| {
			agnostics.device.create_bind_group(&BindGroupDescriptor {
				label: Some("view trace bindgroup"),
				layout: &view_trace_pipeline.get_bind_group_layout(GROUP_INDEX),
				entries: &[
					BindGroupEntry {
						binding: DAG_INDEX,
						resource: dag_buffer.as_entire_binding(),
					},
					BindGroupEntry {
						binding: VIEW_TRACE_INPUT_INDEX,
						resource: view_input_uniform.as_entire_binding(),
					},
					BindGroupEntry {
						binding: VIEW_DATA_INDEX,
						resource: view.as_entire_binding(),
					},
					BindGroupEntry {
						binding: OUTPUT_TEXTURE_INDEX,
						resource: BindingResource::TextureView(&output_texture.create_view(&wgpu::TextureViewDescriptor::default())),
					},
				],
			})
		};
		let view_trace_bindgroups = [create_view_trace_bindgroup(&view_buffers[0]), create_view_trace_bindgroup(&view_buffers[1])];


		/*
		let create_final_bindgroup = || {
			todo!();
		};
		*/


		return Self {
         agnostics: agnostics,

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

		let mut encoder = self.agnostics.device.create_command_encoder(&CommandEncoderDescriptor{label: Some("view trace render pass encoder")});
		
		let mut view_trace_pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some("view trace pass")});
		view_trace_pass.set_pipeline(&self.view_trace_pipeline);
		view_trace_pass.set_bind_group(GROUP_INDEX, &self.view_trace_bindgroups[(self.frame_counter % 2) as usize], &[]);
		view_trace_pass.dispatch_workgroups(self.agnostics.surface_config.width / WORK_GROUP_WIDTH, self.agnostics.surface_config.height / WORK_GROUP_HEIGHT, 1);

		drop(view_trace_pass);


		let surface_texture = self.agnostics.surface.get_current_texture()?;
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
				width: self.agnostics.surface_config.width,
				height: self.agnostics.surface_config.height,
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
		self.agnostics.queue.submit(std::iter::once(encoder.finish()));
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
		self.agnostics.queue.write_buffer(&self.view_input_uniform, 0, unsafe{ std::slice::from_raw_parts((&pos as *const ViewInputData) as *const u8, std::mem::size_of::<ViewInputData>()) });
	}


	pub fn print_state(&self) {
		println!("Device: {}", self.agnostics.adapter.get_info().name);
		println!("Backend: {}", match self.agnostics.adapter.get_info().backend{
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

impl RenderAgnostics {
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
		
		let surface_capabilities = surface.get_capabilities(&adapter);
		let surface_format = surface_capabilities.formats.iter().copied()
			.filter(|f| f.describe().srgb)
			.next().unwrap_or(surface_capabilities.formats[0]);
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
#[allow(unused_macros)]
macro_rules! shaderConstsFormat {
	() => {
		todo!()		
	};
}