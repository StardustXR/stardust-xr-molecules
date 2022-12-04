use crate::single_actor_action::SingleActorAction;
use glam::Vec3;
use stardust_xr_fusion::{
	fields::Field,
	input::{
		action::{BaseInputAction, InputAction, InputActionHandler},
		InputDataType, InputHandler,
	},
	node::{ClientOwned, NodeError},
	spatial::Spatial,
	HandlerWrapper,
};

#[derive(Debug, Clone)]
pub struct GrabData {
	max_distance: f32,
}

pub struct Grabbable {
	root: Spatial,
	content_parent: Spatial,
	global_action: BaseInputAction<GrabData>,
	condition_action: BaseInputAction<GrabData>,
	grab_action: SingleActorAction<GrabData>,
	input_handler: HandlerWrapper<InputHandler, InputActionHandler<GrabData>>,
	min_distance: f32,
}
impl Grabbable {
	pub fn new<Fi: Field + ClientOwned>(
		parent: &Spatial,
		field: &Fi,
		max_distance: f32,
	) -> Result<Self, NodeError> {
		let global_action = BaseInputAction::new(false, |_, _| true);
		let condition_action = BaseInputAction::new(false, |input, data: &GrabData| {
			input.distance < data.max_distance
		});
		let grab_action = SingleActorAction::new(
			true,
			|data, _| {
				data.datamap.with_data(|datamap| match &data.input {
					InputDataType::Hand(_) => datamap.idx("pinch_strength").as_f32() > 0.90,
					_ => datamap.idx("grab").as_f32() > 0.90,
				})
			},
			false,
		);
		let input_handler = InputHandler::create(parent, None, None, field)?
			.wrap(InputActionHandler::new(GrabData { max_distance }))?;
		let root = Spatial::builder()
			.spatial_parent(input_handler.node())
			.zoneable(false)
			.build()?;
		let content_parent = Spatial::builder()
			.spatial_parent(&input_handler.node())
			.zoneable(true)
			.build()?;

		Ok(Grabbable {
			root,
			content_parent,
			global_action,
			condition_action,
			grab_action,
			input_handler,
			min_distance: f32::MAX,
		})
	}
	pub fn update(&mut self) {
		self.input_handler.lock_wrapped().update_actions([
			self.global_action.type_erase(),
			self.condition_action.type_erase(),
			self.grab_action.type_erase(),
		]);
		self.grab_action.update(&mut self.condition_action);

		if let Some(actor) = self.grab_action.actor() {
			match &actor.input {
				InputDataType::Hand(h) => {
					let thumb_tip_pos: Vec3 = h.thumb.tip.position.into();
					let index_tip_pos: Vec3 = h.index.tip.position.into();
					let pinch_pos = thumb_tip_pos.lerp(index_tip_pos, 0.5);
					self.root
						.set_transform(
							Some(self.input_handler.node()),
							Some(pinch_pos.into()),
							Some(h.palm.rotation.clone().into()),
							None,
						)
						.unwrap();
				}
				InputDataType::Pointer(p) => {
					self.root
						.set_transform(
							Some(self.input_handler.node()),
							Some(p.origin),
							Some(p.orientation),
							None,
						)
						.unwrap();
				}
				InputDataType::Tip(t) => {
					self.root
						.set_transform(
							Some(self.input_handler.node()),
							Some(t.origin.into()),
							Some(t.orientation.into()),
							None,
						)
						.unwrap();
				}
			}
		}
		if self.grab_action.actor_started() {
			self.content_parent.set_zoneable(false).unwrap();
			self.content_parent
				.set_spatial_parent_in_place(&self.root)
				.unwrap();
		}
		if self.grab_action.actor_stopped() {
			self.content_parent
				.set_spatial_parent_in_place(self.input_handler.node())
				.unwrap();
			self.content_parent.set_zoneable(true).unwrap();
		}

		self.min_distance = self
			.global_action
			.actively_acting
			.iter()
			.map(|data| data.distance)
			.reduce(|a, b| a.min(b))
			.unwrap_or(f32::MAX);
	}
	pub fn grab_action(&self) -> &SingleActorAction<GrabData> {
		&self.grab_action
	}
	pub fn content_parent(&self) -> &Spatial {
		&self.content_parent
	}
	pub fn min_distance(&self) -> f32 {
		self.min_distance
	}
}
