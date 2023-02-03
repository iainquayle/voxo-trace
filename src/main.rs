#[macro_use] extern crate log;
extern crate env_logger;

use log::Level;
use oct_dag::oct_dag::TestDagType;
use winit::{event::*, event_loop::{ControlFlow, EventLoop}, window::{WindowBuilder}, dpi::{PhysicalSize}, platform::run_return::EventLoopExtRunReturn,};

use crate::oct_dag::oct_dag::OctDag;

pub mod render_engine;
pub mod logic_engine;
pub mod oct_dag;
pub mod io;

const WINDOW_WIDTH: u16 = 1920; 
const WINDOW_HEIGHT: u16 = 1080;

fn main() {
	env_logger::init();

    //written from nvim	

	let mut event_loop = EventLoop::new();
	let window = WindowBuilder::new().with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT)).with_title("Test").with_resizable(false).build(&event_loop).expect("fail to build window");
	//window.set_cursor_grab(true).expect("unable to grab cursor");

	let mut logic = logic_engine::logic_engine::LogicEngine::new(OctDag::new_test(TestDagType::PILLAR, 8));
		
	logic.dag.print_size();
	let mut render = render_engine::render_engine::RenderEngine::new(&window, &logic);
	render.print_state();


	event_loop.run_return(move |event, _, control_flow| {
		match event {
			Event::RedrawRequested(window_id) if window_id == window.id() => {
				match render.render(&logic) {
					Ok(_) => {},
					Err(wgpu::SurfaceError::Lost) => {*control_flow = ControlFlow::Exit},
					Err(wgpu::SurfaceError::OutOfMemory) => {*control_flow = ControlFlow::Exit},
					Err(e) => eprintln!("{:?}", e),
				}
			}
			Event::MainEventsCleared => {
				window.request_redraw();
				logic.update();
			}
			Event::WindowEvent {
				ref event,
				window_id,
			} 
			if window_id == window.id() => {
				logic.input(&window, event);

				match event {
					WindowEvent::CloseRequested => {*control_flow = ControlFlow::Exit},

					/*
					window resizing calls here in tut
					*/
					_ => {}
				}
			},
			_ => {}
		}
	});
}
