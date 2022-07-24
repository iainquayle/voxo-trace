pub mod logic_engine {
	use glam::{Vec3A};
	use std::time::{Instant};
	use winit::{event::{WindowEvent, KeyboardInput, ElementState, VirtualKeyCode}, dpi::{PhysicalPosition}, window::Window};
	use crate::oct_dag::oct_dag::OctDag;

	
	pub struct LogicEngine {
		pub dag: OctDag,
		pub start_time: Instant,
		last_time: u64,

		camera_force: Vec3A,

		pub camera_pos: Vec3A,
		camera_vel: Vec3A,
		pub camera_yaw: f32,
		pub camera_pitch: f32,
		pub camera_roll: f32,
		mouse_sens: f32,

		pub light_pos: Vec3A,
		move_speed: f32, //units / ms
		move_drag: f32,

		pause: bool,
	}

	impl LogicEngine {
		pub fn new(dag: OctDag) -> Self {
			return LogicEngine { dag: dag,
				start_time: Instant::now(),
				last_time: 0,

				camera_force: Vec3A::ZERO,

				camera_pos: Vec3A::ZERO,
				camera_vel: Vec3A::ZERO, 
				camera_yaw: 0.0,
				camera_pitch: 0.0,
				camera_roll: 0.0,
				mouse_sens: 0.0003,

				light_pos: Vec3A::ZERO,
				move_speed: 320.0,
				move_drag: 0.2,

				pause: true,
			};	
		}

		pub fn update(&mut self) {
			let ms_delta = self.start_time.elapsed().as_millis() as u64 - self.last_time;
			self.last_time += ms_delta;

			
			let mut rotated_force = Vec3A::new(self.camera_force.x, 
				self.camera_force.y * self.camera_pitch.cos() + self.camera_force.z * self.camera_pitch.sin(), 
				self.camera_force.z * self.camera_pitch.cos() - self.camera_force.y * self.camera_pitch.sin());
			rotated_force = Vec3A::new(rotated_force.x * self.camera_yaw.cos() - rotated_force.z * self.camera_yaw.sin(),
				rotated_force.y,
				rotated_force.z * self.camera_yaw.cos() + rotated_force.x * self.camera_yaw.sin());
			self.camera_vel = Vec3A::from([self.move_drag; 3]) * (self.camera_vel + rotated_force);
			self.camera_pos = self.camera_pos + self.camera_vel;		
		}

		pub fn input(&mut self, window: &Window, event: &WindowEvent) {	
			match event {
				WindowEvent::KeyboardInput {
					input: KeyboardInput {
						state,
						virtual_keycode: Some(keycode),
						..
					},
					..
				} => {
					if *state == ElementState::Pressed {
						match keycode {
							VirtualKeyCode::Escape => {
								self.pause = !self.pause;
								window.set_cursor_visible(self.pause);
							}
							VirtualKeyCode::W => {
								self.camera_force.z = self.move_speed;
							}
							VirtualKeyCode::S => {
								self.camera_force.z = -self.move_speed;
							}
							VirtualKeyCode::D => {
								self.camera_force.x = self.move_speed;
							}
							VirtualKeyCode::A => {
								self.camera_force.x = -self.move_speed;
							}
							VirtualKeyCode::Space => {
								self.camera_force.y = self.move_speed;
							}
							VirtualKeyCode::LShift => {
								self.camera_force.y = -self.move_speed;
							}
							_ => {},
						}
					} else if *state == ElementState::Released {
						match keycode {
							VirtualKeyCode::W | VirtualKeyCode::S => {
								self.camera_force.z = 0.0;
							}
							VirtualKeyCode::A | VirtualKeyCode::D => {
								self.camera_force.x = 0.0;
							}
							VirtualKeyCode::Space | VirtualKeyCode::LShift => {
								self.camera_force.y = 0.0;
							}
							_ => {},
						}
					}
				},
				WindowEvent::CursorMoved { 
					position: PhysicalPosition { x, y }, 
				..} => {
					if !self.pause {
						self.camera_yaw += (((window.inner_size().width / 2) as f64 - x) as f32) * self.mouse_sens;
						self.camera_yaw %= 2.0 * std::f32::consts::PI;

						let new_pitch = self.camera_pitch + (((window.inner_size().height / 2) as f64 - y) as f32) * self.mouse_sens;
						if new_pitch > std::f32::consts::PI {
							self.camera_pitch = std::f32::consts::PI;
						} else if new_pitch < -std::f32::consts::PI {
							self.camera_pitch = -std::f32::consts::PI;
						} else {
							self.camera_pitch = new_pitch;
						}

						window.set_cursor_position(PhysicalPosition::new(window.inner_size().width / 2, window.inner_size().height / 2)).ok();
					}
				}
				_ => {},
			}
		}
	}
}