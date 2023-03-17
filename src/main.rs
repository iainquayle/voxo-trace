#![allow(dead_code)]
//#[macro_use] extern crate log;
extern crate env_logger;

//use log::Level;
use winit::{event::*, 
	event_loop::{ControlFlow, EventLoop},
	platform::run_return::EventLoopExtRunReturn,};

mod window;
mod render_engine;
mod logic_engine;
mod oct_dag;
mod io;

use crate::{oct_dag::{OctDag, TestDagType},
	logic_engine::LogicEngine,
	render_engine::RenderEngine,
	window::Window};

const WINDOW_WIDTH: u16 = 1920; 
const WINDOW_HEIGHT: u16 = 1080;

fn main() {
	env_logger::init();

	let mut event_loop = EventLoop::new();
	let mut window = Window::new(WINDOW_WIDTH, WINDOW_HEIGHT, &event_loop);	

	let mut logic = LogicEngine::new(OctDag::new_test(TestDagType::PILLAR, 6));
	logic.dag.print_size();
		
	let mut render = RenderEngine::new(&window, &logic);
	render.print_state();

	event_loop.run_return(move |event, _, control_flow| {
		match event {
			Event::RedrawRequested(window_id) if window_id == window.id() => {
				match render.render(&logic) {
					Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::OutOfMemory) => 
						{*control_flow = ControlFlow::Exit},
					Err(e) => eprintln!("{:?}", e),
					Ok(_) => {},
				}
			}
			Event::MainEventsCleared => {
				window.request_redraw();
				logic.update(&window);
			}
			Event::WindowEvent {
				ref event,
				window_id,
			} if window_id == window.id() => {
				window.record_events(event);
				match event {
					WindowEvent::CloseRequested => {*control_flow = ControlFlow::Exit},
					_ => {}
				}
			},
			_ => {}
		}
	});
}
