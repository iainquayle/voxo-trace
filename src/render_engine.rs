pub mod render_engine {
	use pollster::FutureExt;
	use wgpu::{include_wgsl, util::DeviceExt};
	use winit::{window::{Window},};
	use glam::{Vec3, Vec4, UVec4};
	use crate::{oct_dag::{oct_dag::{Node}}, logic_engine::logic_engine::LogicEngine};



	/*
	read only: 0-99
	inputs: 100-199
	intermediates: 200-299
	output: 300
	 */
	enum BindIndeces {
		Dag = 0,
		TemporalInput = 102,
		ViewInput = 100,
		ViewData = 200,
	}
	const GROUP_INDEX: u32 = 0;
	const DAG_INDEX: u32 = 0;
	const TEMPORAL_INPUT_INDEX: u32 = 102;
	const VIEW_TRACE_INPUT_INDEX: u32 = 100;
	const LIGHT_TRACE_INPUT_INDEX: u32 = 101;
	const VIEW_DATA_INDEX: u32 = 200;
	const LIGHT_DATA_INDEX: u32 = 201;
	const OUTPUT_TEXTURE_INDEX: u32 = 300;

	const WORK_GROUP_WIDTH: u32 = 8;
	const WORK_GROUP_HEIGHT: u32 = 8;

	const LIGHT_TEXTURE_WIDTH: u32 = 1024;
	const LIGHT_TEXTURE_HEIGHT: u32 = 1024;

	//TODO: move shaders all into one file for bidnings ease of managment
	//TODO: see about making a macro that changes the string to automatically input the work group dimensions and othe r conts
	macro_rules! SHADERS_LOC {() => {"shaders.wgsl"};}
	macro_rules! VIEW_TRACE_ENTRY {() => {"view_trace"};}

	/*
	move temporals to their own uniform structure, no need to have them copied ina bunch of different inputs, unless it is used as a way to progress animations at different rates
	 */
	pub struct TemporalInputData {
		pub temporals: UVec4,
	}
	pub struct ViewInputData {
		pub pos: Vec4, //x, y, z, pad
		pub rads: Vec4, //yaw, pitch, roll, pad(change to fov)
		pub light_pos: Vec4, //remove
		//chnage so that the 2nd element is the fov, chnage name to something else?
		pub temporals: UVec4, //time, frame, pad, pad
	}
	pub struct LightInputData {
		pub pos: Vec4,//x y z pad
		pub rads: Vec4,//yaw, ptch, roll, fov //change to just dir? doesnt matter much, the amount of time spent on calculating dir from this would be trivial
		pub rgb: Vec4,//r g b pad //a could be used to increase itensity?
		pub temporals: UVec4,//time, frame, pad, pad 
	}

	/*
	these structs are never used out side of the device, they are here only so that the correct memory size can be allocated
	 */
	struct ViewData {
		pub _pos: Vec3,
		pub _len: f32,
		pub _rgba: u32, 		
		pub _normal: u32, 
	}
	struct LightData {
		pub _rgb_len: Vec3,
		pub _rgb: Vec3,
	}

	pub struct RenderEngine {
		surface: wgpu::Surface,
		//instance: wgpu::Instance,
		adapter: wgpu::Adapter,
		device: wgpu::Device,
		queue: wgpu::Queue,
		surface_config: wgpu::SurfaceConfiguration,

		//compute_shader:
		//view_trace_layout: wgpu::PipelineLayout,
		view_trace_pipeline: wgpu::ComputePipeline,

		view_trace_bindgroups: [wgpu::BindGroup; 2],
		//output_view: wgpu::TextureView,

		//dag_buffer: wgpu::Buffer,
		view_input_uniform: wgpu::Buffer,
		output_texture: wgpu::Texture,

		//camera: ViewInputData,
		frame_counter: u64,
		fps_prev_time: u128,
	}

	impl RenderEngine {
		pub fn new(window: &Window, state: &LogicEngine) -> Self {
			let instance = wgpu::Instance::new(wgpu::Backends::DX12);
			let surface = unsafe{instance.create_surface(window)};

			let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions{
				power_preference: wgpu::PowerPreference::HighPerformance,
				compatible_surface: Some(&surface),
				force_fallback_adapter: false,
			}).block_on().expect("Fail to find suitable adapter");
			
			let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor{
				features: wgpu::Features::default(),
				limits: wgpu::Limits{..Default::default()},
				label: Some("device"),
			}, None).block_on().expect("failed to create device and queue");

			let surface_config = wgpu::SurfaceConfiguration {
				usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
				format: wgpu::TextureFormat::Rgba8Unorm,//surface.get_preferred_format(&adapter).unwrap()
				width: window.inner_size().width,
				height: window.inner_size().height,
				present_mode: wgpu::PresentMode::Immediate,
			};
			surface.configure(&device, &surface_config);



			/*
			create buffers
			 */
			let view_input_uniform = device.create_buffer( &wgpu::BufferDescriptor {
				label: Some(" buffer"),
				mapped_at_creation: false,
				usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
				size: std::mem::size_of::<ViewInputData>() as u64,
			});
			let byte_dag_array = unsafe{std::slice::from_raw_parts(state.dag.nodes[..].as_ptr() as *const u8, std::mem::size_of::<Node>() * state.dag.nodes.len())};
			let dag_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor{
				label: Some("dag buffer"),
				usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
				contents: byte_dag_array,
			});
			let output_texture = device.create_texture(&wgpu::TextureDescriptor {
				label: Some("output texture"),
				size: wgpu::Extent3d {
					width: window.inner_size().width,
					height: window.inner_size().height,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba8Unorm,
				usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
			});
			//returns buffers required to make one lane in double buffered pipline
			//does not need to create the input uniform or buffers or the output texture
			let create_parallel_buffers = || {
				let create_view_buffer= || {
					device.create_buffer( &wgpu::BufferDescriptor {
						label: Some("view data buffer"),
						mapped_at_creation: false,
						usage: wgpu::BufferUsages::STORAGE,
						size: (surface_config.height * surface_config.width * std::mem::size_of::<ViewData>() as u32) as u64,
					})
				};
				/*
				light buffer when creating multiples will either need to be made as multiple buffers, or one large buffer
				should be ok since the buffer can be treated as a muti dimensionsonal array once it is in the shader
				 */
				let create_light_buffer= || {
					device.create_buffer( &wgpu::BufferDescriptor {
						label: Some(&format!("light buffer {}", 0)),
						mapped_at_creation: false,
						usage: wgpu::BufferUsages::STORAGE,
						size: (LIGHT_TEXTURE_WIDTH * LIGHT_TEXTURE_HEIGHT * std::mem::size_of::<LightData>() as u32) as u64,
					})
				};

				([create_view_buffer(), create_view_buffer()], [create_light_buffer(), create_light_buffer()])
			};
			let (view_buffers, light_buffers) = create_parallel_buffers();


			/*
			create view trace pipeline
			TODO: this should be able to be turned into just a singular pipline layout, not differentiated as view trace
			 */
			let view_trace_shader = device.create_shader_module(&include_wgsl!(SHADERS_LOC!()));
			let view_trace_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {//not sure if this is necessary when in this compute case, ie simple single stage shader no texture sampling required...
				label: Some("view trace pipline layout"),
				push_constant_ranges: &[],
				bind_group_layouts: &[
					&device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
						label: Some("view trace bind group layout"),
						entries: &[
							wgpu::BindGroupLayoutEntry {
								binding: DAG_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Storage { read_only: true }, 
									has_dynamic_offset: false, 
									min_binding_size: None, 
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: VIEW_TRACE_INPUT_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty : wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Uniform, 
									has_dynamic_offset: false, 
									min_binding_size: None,
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: VIEW_DATA_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Storage { read_only: false }, 
									has_dynamic_offset: false, 
									min_binding_size: None 
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: OUTPUT_TEXTURE_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::StorageTexture { 
									access: wgpu::StorageTextureAccess::WriteOnly, 
									format: wgpu::TextureFormat::Rgba8Unorm, 
									view_dimension: wgpu::TextureViewDimension::D2 
								},
								count: None,
							},
						],
					}),
				],
			});
			let view_trace_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
				label: Some("view trace pipeline"),
				layout: Some(&view_trace_layout),
				module: &view_trace_shader,
				entry_point: VIEW_TRACE_ENTRY!(), 
			});	

			let create_view_trace_bindgroup = |view: &wgpu::Buffer| {
				device.create_bind_group(&wgpu::BindGroupDescriptor {
					label: Some("view trace bindgroup"),
					layout: &view_trace_pipeline.get_bind_group_layout(GROUP_INDEX),
					entries: &[
						wgpu::BindGroupEntry {
							binding: DAG_INDEX,
							resource: dag_buffer.as_entire_binding(),
						},
						wgpu::BindGroupEntry {
							binding: VIEW_TRACE_INPUT_INDEX,
							resource: view_input_uniform.as_entire_binding(),
						},
						wgpu::BindGroupEntry {
							binding: VIEW_DATA_INDEX,
							resource: view.as_entire_binding(),
						},
						wgpu::BindGroupEntry {
							binding: OUTPUT_TEXTURE_INDEX,
							resource: wgpu::BindingResource::TextureView(&output_texture.create_view(&wgpu::TextureViewDescriptor::default())),
						},
					],
				})
			};
			let view_trace_bindgroups = [create_view_trace_bindgroup(&view_buffers[0]), create_view_trace_bindgroup(&view_buffers[1])];



			/*
			light trace pipline start
			 */
			/*
			let light_trace_shader = device.create_shader_module(&include_wgsl!(LIGHT_TRACE_SHADER_LOC!()));
			let light_trace_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {//not sure if this is necessary when in this compute case, ie simple single stage shader no texture sampling required...
				label: Some("light trace pipline layout"),
				push_constant_ranges: &[],
				bind_group_layouts: &[
					&device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
						label: Some("light trace bind group layout"),
						entries: &[ 
							wgpu::BindGroupLayoutEntry {
								binding: DAG_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Storage { read_only: true }, 
									has_dynamic_offset: false, 
									min_binding_size: None, 
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: LIGHT_TRACE_INPUT_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty : wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Uniform, 
									has_dynamic_offset: false, 
									min_binding_size: None,
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: VIEW_DATA_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Storage { read_only: true }, 
									has_dynamic_offset: false, 
									min_binding_size: None 
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: LIGHT_DATA_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::Buffer { 
									ty: wgpu::BufferBindingType::Storage { read_only: false }, 
									has_dynamic_offset: false, 
									min_binding_size: None 
								},
								count: None,
							},
							wgpu::BindGroupLayoutEntry {
								binding: OUTPUT_TEXTURE_INDEX,
								visibility: wgpu::ShaderStages::COMPUTE,
								ty: wgpu::BindingType::StorageTexture { 
									access: wgpu::StorageTextureAccess::WriteOnly, 
									format: wgpu::TextureFormat::Rgba8Unorm, 
									view_dimension: wgpu::TextureViewDimension::D2 
								},
								count: None,
							},
						],
					}),
				],
			});
			let light_trace_pipline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
				label: Some("light trade pipeline"),
				layout: Some(&light_trace_layout),
				module: &light_trace_shader,
				entry_point: "main",
			});
			
			let create_light_trace_bindgroup = || {
				todo!();
			};
			*/


			let create_final_bindgroup = || {
				todo!();
			};


			return RenderEngine {
				surface: surface,
				//instance: instance,
				adapter: adapter,
				device: device,
				queue: queue,
				surface_config: surface_config,

				view_trace_pipeline: view_trace_pipeline,
				view_trace_bindgroups: view_trace_bindgroups,
				//output_view: output_view,

				//dag_buffer: dag_buffer,
				output_texture: output_texture,
				view_input_uniform: view_input_uniform,

				//camera: camera,
				frame_counter: 0,
				fps_prev_time: 0,			
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
		pub fn render(&mut self, state: &LogicEngine) -> Result<(), wgpu::SurfaceError> {
				self.update_camera(ViewInputData { 
				pos: Into::<Vec4>::into((state.camera_pos, 0.0)),
				rads: Vec4::new(state.camera_yaw, state.camera_pitch, state.camera_roll, 0.0),
				light_pos: Into::<Vec4>::into((state.light_pos, 0.0)),
				temporals: UVec4::new(state.start_time.elapsed().as_millis() as u32, self.frame_counter as u32, 0, 0),
			});
	
			let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{label: Some("view trace render pass encoder")});
			
			let mut view_trace_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("view trace pass")});
			view_trace_pass.set_pipeline(&self.view_trace_pipeline);
			view_trace_pass.set_bind_group(GROUP_INDEX, &self.view_trace_bindgroups[(self.frame_counter % 2) as usize], &[]);
			view_trace_pass.dispatch(self.surface_config.width / WORK_GROUP_WIDTH, self.surface_config.height / WORK_GROUP_HEIGHT, 1);

			drop(view_trace_pass);

			let surface_texture = self.surface.get_current_texture()?;
			encoder.copy_texture_to_texture(
				wgpu::ImageCopyTexture {
					aspect: wgpu::TextureAspect::All,
					texture: &self.output_texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
				},
				wgpu::ImageCopyTexture {
					aspect: wgpu::TextureAspect::All,
					texture: &surface_texture.texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
				},
				wgpu::Extent3d {
					width: self.surface_config.width,
					height: self.surface_config.height,
					depth_or_array_layers: 1,
				},
			);

			//if wnating to make a multi stage rendering process, create second encoder, and submitt both at once
			//the second will need to operate on data given by the previous iteration rather than the same
			self.queue.submit(std::iter::once(encoder.finish()));
			surface_texture.present();

			/*
			 * not currently perfectly accurate, but does approximate the correct fps given some time
			 */
			self.frame_counter += 1;
			if self.frame_counter % 500 == 0 {
				println!("fps: {}", 500.0 / ((state.start_time.elapsed().as_millis() - self.fps_prev_time) as f32 / 1000.0));
				self.fps_prev_time = state.start_time.elapsed().as_millis();
			}

			return Ok(());
		}

		pub fn update_dag(&mut self) {}

		pub fn update_camera(&mut self, pos: ViewInputData) {
			self.queue.write_buffer(&self.view_input_uniform, 0, unsafe{ std::slice::from_raw_parts((&pos as *const ViewInputData) as *const u8, std::mem::size_of::<ViewInputData>()) });
		}


		pub fn print_state(&self) {
			println!("Device: {}", self.adapter.get_info().name);
			println!("Backend: {}", match self.adapter.get_info().backend{
				wgpu::Backend::BrowserWebGpu => "Browser",
				wgpu::Backend::Dx12 => "DX12",
				wgpu::Backend::Vulkan => "Vulkan",
				wgpu::Backend::Metal => "Metal",
				wgpu::Backend::Dx11 => "DX11",
				wgpu::Backend::Gl => "OpenGL",
				wgpu::Backend::Empty => "None"
			});
		}
	}

	macro_rules! shaderConstsFormat {
		() => {
			
		};
	}
}