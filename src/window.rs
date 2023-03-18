use std::collections::HashSet;
use winit::{event::{WindowEvent, KeyboardInput, ElementState, VirtualKeyCode}, 
	event_loop::{EventLoop},
	window::{WindowBuilder, WindowId, Window as WinitWindow},
	dpi::{PhysicalSize, PhysicalPosition}, };
use glam::{Vec2};

pub struct Window {
	window: WinitWindow, 
	pressed_keys: HashSet<VirtualKeyCode>,
	//will need to be changed i future if threaded
	cursor_position: Vec2,
	cursor_captured: bool,
}

impl Window {
	pub fn new(width: u16, height: u16, event_loop: &EventLoop<()>) -> Self {
		Self {
			window: WindowBuilder::new()
				.with_inner_size(PhysicalSize::new(width, height))
				.with_title("VoxoTrace")
				.with_resizable(false)
				.build(event_loop)
				.expect("fail to build window") , 
			pressed_keys: HashSet::new(),
			cursor_position: Vec2::ZERO,
			cursor_captured: false,
		}
	}
	//future use if moved to multithreaded
	pub fn event_loop(&mut self) {
	}
	pub fn record_events(&mut self, event: &WindowEvent) {
      self.cursor_position = Vec2::ZERO;
		match event {
			WindowEvent::KeyboardInput {
				input: KeyboardInput {
					state,
					virtual_keycode: Some(keycode),
					..
				},
				..
			} => {
				match state {
					ElementState::Pressed => {
						self.pressed_keys.insert(keycode.clone());
						if let VirtualKeyCode::Escape = keycode {
							self.cursor_captured = !self.cursor_captured;
							if self.cursor_captured {
								self.center_cursor();
							}
						}
					},
					ElementState::Released => {
						self.pressed_keys.remove(keycode);							
					},
				}	
			},
			WindowEvent::CursorMoved { 
				position: PhysicalPosition { x, y }, 
			..} => {
				self.cursor_position = Vec2::new(((self.window.inner_size().width / 2) as f64 - x) as f32, ((self.window.inner_size().height / 2) as f64 - y) as f32);
				self.window.set_cursor_visible(!self.cursor_captured);
				if self.cursor_captured {
					self.center_cursor();
				}
			},
			_ => {},
		}	
	} 
	fn center_cursor(&self) {
		self.window.set_cursor_position(PhysicalPosition::new(
			self.window.inner_size().width / 2, self.window.inner_size().height / 2)).ok();
	}
	pub fn key_pressed(&self, keycode: VirtualKeyCode) -> bool {
		self.pressed_keys.contains(&keycode)
	}
	pub fn request_redraw(&self) {
		self.window.request_redraw();
	}
	pub fn capture_cursor(&mut self, grab_cursor: bool) {
		self.cursor_captured = grab_cursor;	
	}
	pub fn cursor_position(&self) -> Vec2 {
		self.cursor_position
	}
	pub fn borrow_window(&self) -> &WinitWindow {
		&self.window
	}
	pub fn cursor_captured(&self) -> bool {
		self.cursor_captured
	}
	pub fn inner_width(&self) -> u32 {
		self.window.inner_size().width	
	}
	pub fn inner_height(&self) -> u32 {
		self.window.inner_size().height	
	}
	pub fn id(&self) -> WindowId {
		self.window.id()
	}
}