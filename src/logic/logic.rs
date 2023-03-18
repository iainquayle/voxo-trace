use glam::{Vec3A, Vec3};
use std::time::{Instant};
use winit::event::VirtualKeyCode;
use crate::{asset::oct_dag::OctDag, window::Window};

#[derive(Default, Clone, Copy)]
pub struct CameraPose {
	pub force: Vec3A,
   pub position: Vec3A,
	pub velocity: Vec3A,

	pub yaw: f32,
	pub pitch: f32, 
	pub roll: f32,
}
pub struct Logic {
	pub dag: OctDag,
	pub start_time: Instant,
	last_time: u64,

	movement_keys: Vec<(VirtualKeyCode, Vec3A)>,

	mouse_sens: f32,
	move_speed: f32, //units / ms
	move_drag: f32,

	camera_pose: CameraPose,


}

impl Logic {
	pub fn new(dag: OctDag) -> Self {
		return Self { 
			dag: dag,
			start_time: Instant::now(),
			last_time: 0,

			mouse_sens: 0.0003,
			move_speed: 320.0,
			move_drag: 0.2,
			
			movement_keys: vec![
				(VirtualKeyCode::W, Vec3A::new(0.0, 0.0, 1.0)),
				(VirtualKeyCode::S, Vec3A::new(0.0, 0.0, -1.0)),
				(VirtualKeyCode::A, Vec3A::new(-1.0, 0.0, 0.0)),
				(VirtualKeyCode::D, Vec3A::new(1.0, 0.0, 0.0)),
				(VirtualKeyCode::Space, Vec3A::new(0.0, 1.0, 0.0)),
				(VirtualKeyCode::LShift, Vec3A::new(0.0, -1.0, 0.0)),
			],

			camera_pose: Default::default(),
		};	
	}

	pub fn update(&mut self, window: &Window) {
		let ms_delta = self.start_time.elapsed().as_millis() as u64 - self.last_time;
		self.last_time += ms_delta;
        
		if window.cursor_captured() {
			//TODO: dont need member force anymore?
			self.camera_pose.force = Vec3A::ZERO;
			for (keycode, force) in &self.movement_keys {
				if window.key_pressed(keycode.clone()) {
					//println!("here");
					self.camera_pose.force += force.clone() * Vec3A::splat(self.move_speed);
				}
			}

			self.camera_pose.yaw += window.cursor_position().x * self.mouse_sens; 
			self.camera_pose.yaw %= 2.0 * std::f32::consts::PI;

			let new_pitch = self.camera_pose.pitch 
				+ window.cursor_position().y 
				* self.mouse_sens;
			self.camera_pose.pitch = if new_pitch > std::f32::consts::PI / 2.0 { 
				std::f32::consts::PI / 2.0
			} else if new_pitch < -std::f32::consts::PI / 2.0 {
				-std::f32::consts::PI / 2.0
			} else {
				new_pitch
			};
		}

			
		let mut rotated_force = Vec3A::new(self.camera_pose.force.x, 
			self.camera_pose.force.y * self.camera_pose.pitch.cos() + self.camera_pose.force.z * self.camera_pose.pitch.sin(), 
			self.camera_pose.force.z * self.camera_pose.pitch.cos() - self.camera_pose.force.y * self.camera_pose.pitch.sin());
		rotated_force = Vec3A::new(rotated_force.x * self.camera_pose.yaw.cos() - rotated_force.z * self.camera_pose.yaw.sin(),
			rotated_force.y,
			rotated_force.z * self.camera_pose.yaw.cos() + rotated_force.x * self.camera_pose.yaw.sin());
		self.camera_pose.velocity = Vec3A::splat(self.move_drag) * (self.camera_pose.velocity + rotated_force);
		self.camera_pose.position += self.camera_pose.velocity;		
	}
    
	pub fn camera_pose(&self) -> CameraPose {
		self.camera_pose
	}
	pub fn camera_orientaion_vec3(&self) -> Vec3 {
		Vec3::new(self.camera_pose.yaw, self.camera_pose.pitch, self.camera_pose.roll)
	}
}