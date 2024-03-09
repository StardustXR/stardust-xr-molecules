use crate::{
	input_action::{BaseInputAction, InputActionHandler, SingleActorAction},
	lines::{self, LineExt},
	DebugSettings, VisualDebug,
};
use color::{color_space::LinearRgb, rgba_linear, Rgba};
use glam::{vec3, Mat4, Vec3};
use map_range::MapRange;
use mint::{Vector2, Vector3};
use rustc_hash::FxHashSet;
use stardust_xr_fusion::{
	drawable::{Line, LinePoint, Lines, LinesAspect},
	fields::{BoxField, BoxFieldAspect, UnknownField},
	input::{InputData, InputDataType, InputHandler},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
	HandlerWrapper,
};
use std::{ops::Range, sync::Arc};

#[derive(Debug, Clone)]
pub struct HoverPlaneSettings {
	line_start_thickness: f32,
	line_start_color_hover: Rgba<f32, LinearRgb>,
	line_start_color_interact: Rgba<f32, LinearRgb>,
	line_end_thickness: f32,
	line_end_color_hover: Rgba<f32, LinearRgb>,
	line_end_color_interact: Rgba<f32, LinearRgb>,
}
impl Default for HoverPlaneSettings {
	fn default() -> Self {
		HoverPlaneSettings {
			line_start_thickness: 0.0,
			line_start_color_hover: rgba_linear!(1.0, 1.0, 1.0, 1.0),
			line_start_color_interact: rgba_linear!(0.0, 1.0, 0.75, 1.0),
			line_end_thickness: 0.005,
			line_end_color_hover: rgba_linear!(1.0, 1.0, 1.0, 0.0),
			line_end_color_interact: rgba_linear!(0.0, 1.0, 0.75, 0.0),
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct State {
	size: Vector2<f32>,
}

pub struct HoverPlane {
	root: Spatial,
	input: HandlerWrapper<InputHandler, InputActionHandler<State>>,
	field: BoxField,
	hover_action: BaseInputAction<State>,
	interact_action: SingleActorAction<State>,
	size: Vector2<f32>,
	pub x_range: Range<f32>,
	pub y_range: Range<f32>,
	thickness: f32,
	settings: HoverPlaneSettings,
	lines: Lines,
	debug_lines: Option<Lines>,
}
impl HoverPlane {
	pub fn create(
		parent: &impl SpatialAspect,
		transform: Transform,
		size: impl Into<Vector2<f32>>,
		thickness: f32,
		x_range: Range<f32>,
		y_range: Range<f32>,
		settings: HoverPlaneSettings,
	) -> Result<Self, NodeError> {
		let size = size.into();
		let root = Spatial::create(parent, transform, false)?;
		let field = BoxField::create(
			&root,
			Transform::from_translation([0.0, 0.0, thickness * -0.5]),
			[size.x, size.y, thickness],
		)?;
		let input = InputActionHandler::wrap(
			InputHandler::create(&root, Transform::none(), &field)?,
			State { size },
		)?;

		let hover_action = BaseInputAction::new(false, Self::hover_action);
		let interact_action = SingleActorAction::new(true, Self::interact_action, false);

		let lines = Lines::create(&root, Transform::identity(), &[])?;
		Ok(HoverPlane {
			root,
			input,
			field,
			hover_action,
			interact_action,
			size,
			x_range,
			y_range,
			thickness,
			settings,
			lines,
			debug_lines: None,
		})
	}

	fn hover(size: Vector2<f32>, point: Vector3<f32>, front: bool) -> bool {
		point.x.abs() * 2.0 < size.x
			&& point.y.abs() * 2.0 < size.y
			&& point.z.is_sign_positive() == front
	}
	fn hover_action(input: &InputData, state: &State) -> bool {
		match &input.input {
			InputDataType::Pointer(_) => input.distance < 0.0,
			_ => Self::hover(state.size, Self::interact_point_local(input).into(), true),
		}
	}
	fn interact_action(input: &InputData, _state: &State) -> bool {
		match &input.input {
			InputDataType::Hand(_) => input
				.datamap
				.with_data(|d| d.idx("pinch_strength").as_f32() > 0.9),
			_ => input.datamap.with_data(|d| d.idx("select").as_f32() > 0.5),
		}
	}
	pub fn interact_point_local(input: &InputData) -> Vec3 {
		match &input.input {
			InputDataType::Pointer(p) => {
				let normal = vec3(0.0, 0.0, 1.0);
				let denom = normal.dot(p.direction().into());
				let t = -Vec3::from(p.origin).dot(normal) / denom;
				Vec3::from(p.origin) + Vec3::from(p.direction()) * t
			}
			InputDataType::Hand(h) => {
				(Vec3::from(h.index.tip.position) + Vec3::from(h.thumb.tip.position)) * 0.5
			}
			InputDataType::Tip(t) => t.origin.into(),
		}
	}
	pub fn interact_point(&self, input: &InputData) -> (Vector2<f32>, f32) {
		let interact_point = Self::interact_point_local(input);

		let x = interact_point
			.x
			.clamp(-self.size.x / 2.0, self.size.x / 2.0)
			.map_range(-self.size.x / 2.0..self.size.x / 2.0, self.x_range.clone());
		let y = interact_point
			.y
			.clamp(-self.size.y / 2.0, self.size.y / 2.0)
			.map_range(self.size.y / 2.0..-self.size.y / 2.0, self.y_range.clone());

		([x, y].into(), interact_point.z)
	}
	pub fn input_to_points<'a>(
		&self,
		inputs: impl Iterator<Item = &'a Arc<InputData>>,
	) -> Vec<Vector2<f32>> {
		inputs.map(|i| self.interact_point(i).0).collect()
	}

	pub fn root(&self) -> &Spatial {
		&self.root
	}
	pub fn input_handler(&self) -> &InputHandler {
		self.input.node()
	}
	pub fn field(&self) -> UnknownField {
		UnknownField::alias_field(&self.field)
	}

	pub fn set_size(&mut self, size: impl Into<Vector2<f32>>) -> Result<(), NodeError> {
		let size = size.into();
		self.size = size;
		self.input.lock_wrapped().update_state(State { size });
		self.field.set_size([size.x, size.y, self.thickness])?;
		Ok(())
	}
	pub fn set_thickness(&mut self, thickness: f32) -> Result<(), NodeError> {
		self.thickness = thickness;
		self.field
			.set_local_transform(Transform::from_translation([0.0, 0.0, thickness * -0.5]))?;
		self.field.set_size([self.size.x, self.size.y, thickness])?;
		Ok(())
	}

	/// Get all the raw inputs that are hovering
	pub fn hovering_inputs(&self) -> FxHashSet<Arc<InputData>> {
		self.hover_action.currently_acting.clone()
	}
	/// Get all the points hovering over the surface, in x_range and y_range
	pub fn hover_points(&self) -> Vec<Vector2<f32>> {
		self.input_to_points(self.hover_action.currently_acting.iter())
	}

	/// Get the input that's interacting
	pub fn interact_status(&self) -> &SingleActorAction<State> {
		&self.interact_action
	}

	/// Set whether this will receive input or not
	pub fn set_enabled(&self, enabled: bool) -> Result<(), NodeError> {
		self.input.node().set_enabled(enabled)
	}

	/// Update the state of this touch plane. Run once every frame.
	pub fn update(&mut self) {
		self.input
			.lock_wrapped()
			.update_actions([&mut self.hover_action, self.interact_action.base_mut()]);
		self.interact_action.update(Some(&mut self.hover_action));

		let hovered_lines = self
			.hover_action
			.currently_acting
			.iter()
			.chain(self.interact_action.actor())
			.filter(|d| match &d.input {
				InputDataType::Pointer(_) => false,
				_ => true,
			})
			.map(|d| {
				(
					Self::interact_point_local(d),
					self.interact_action.actor() == Some(d),
				)
			})
			.map(|(p, i)| Line {
				points: vec![
					LinePoint {
						point: [
							p.x.clamp(self.size.x * -0.5, self.size.x * 0.5),
							p.y.clamp(self.size.y * -0.5, self.size.y * 0.5),
							0.0,
						]
						.into(),
						thickness: self.settings.line_start_thickness,
						color: if i {
							self.settings.line_start_color_interact
						} else {
							self.settings.line_start_color_hover
						},
					},
					LinePoint {
						point: p.into(),
						thickness: self.settings.line_end_thickness,
						color: if i {
							self.settings.line_end_color_interact
						} else {
							self.settings.line_end_color_hover
						},
					},
				],
				cyclic: false,
			})
			.collect::<Vec<_>>();
		self.lines.set_lines(&hovered_lines).unwrap();
	}
}
impl VisualDebug for HoverPlane {
	fn set_debug(&mut self, settings: Option<DebugSettings>) {
		self.debug_lines = settings.and_then(|settings| {
			let line_front = lines::rounded_rectangle(
				self.size.x,
				self.size.y,
				settings.line_thickness * 0.5,
				4,
			)
			.thickness(settings.line_thickness)
			.color(settings.line_color);
			let line_back = line_front
				.clone()
				.color(rgba_linear!(
					settings.line_color.c.r,
					settings.line_color.c.g,
					settings.line_color.c.b,
					settings.line_color.a * 0.5
				))
				.transform(Mat4::from_translation(vec3(0.0, 0.0, -self.thickness)));

			let lines = Lines::create(
				&self.root,
				Transform::from_translation([0.0, 0.0, 0.0]),
				&[line_front, line_back],
			)
			.ok()?;
			Some(lines)
		})
	}
}
