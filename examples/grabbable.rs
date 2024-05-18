#![allow(dead_code)]

use color_eyre::eyre::Result;
use lazy_static::lazy_static;
use manifest_dir_macros::directory_relative_path;
use stardust_xr_fusion::{
	client::{Client, ClientState, FrameInfo, RootHandler},
	core::values::ResourceID,
	drawable::{Line, Lines, Model},
	fields::BoxField,
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, SpatialRefAspect, Transform},
};
use stardust_xr_molecules::{
	lines::{bounding_box, LineExt},
	Grabbable, GrabbableSettings, PointerMode,
};
use tracing_subscriber::EnvFilter;

lazy_static! {
	static ref GRABBABLE_MODEL: ResourceID = ResourceID::new_namespaced("molecules", "grabbable");
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;
	tracing_subscriber::fmt()
		.compact()
		.with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
		.init();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped_root = client.wrap_root(GrabbableDemo::new(&client).await?)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	}
	Ok(())
}

struct GrabbableDemo {
	state_root: Spatial,
	grabbable: Grabbable,
	field: BoxField,
	model: Model,
	bounding_box: Lines,
}
impl GrabbableDemo {
	async fn new(client: &Client) -> Result<Self, NodeError> {
		let state_root = Spatial::create(client.get_root(), Transform::identity(), false)?;
		let model = Model::create(
			client.get_root(),
			Transform::from_scale([0.5; 3]),
			&*GRABBABLE_MODEL,
		)?;
		let bounds = model.get_relative_bounding_box(client.get_root()).await?;
		let bounding_lines: Vec<Line> = bounding_box(bounds.clone())
			.into_iter()
			.map(|l| l.thickness(0.001))
			.collect();
		let bounding_box = Lines::create(&model, Transform::identity(), &bounding_lines)?;
		let field = BoxField::create(
			&model,
			Transform::from_translation(bounds.center),
			bounds.size,
		)?;

		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::none(),
			&field,
			GrabbableSettings {
				pointer_mode: PointerMode::Align,
				..Default::default()
			},
		)?;
		if let Some(content_parent_reference) = client.state().spatial_anchors.get("content_parent")
		{
			grabbable
				.content_parent()
				.set_relative_transform(content_parent_reference, Transform::identity())?;
		}
		model.set_spatial_parent(grabbable.content_parent())?;
		field.set_spatial_parent(grabbable.content_parent())?;
		bounding_box.set_spatial_parent(grabbable.content_parent())?;

		Ok(GrabbableDemo {
			state_root,
			grabbable,
			field,
			model,
			bounding_box,
		})
	}
}
impl RootHandler for GrabbableDemo {
	fn frame(&mut self, info: FrameInfo) {
		self.grabbable.update(&info).unwrap();
	}
	fn save_state(&mut self) -> ClientState {
		self.state_root
			.set_relative_transform(
				self.grabbable.content_parent(),
				Transform::from_translation([0.0; 3]),
			)
			.unwrap();
		ClientState {
			data: Vec::new(),
			root: self.state_root.alias(),
			spatial_anchors: [(
				"content_parent".to_string(),
				self.grabbable.content_parent().alias(),
			)]
			.into_iter()
			.collect(),
		}
	}
}
