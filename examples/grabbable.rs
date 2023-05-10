#![allow(dead_code)]

use color_eyre::eyre::Result;
use lazy_static::lazy_static;
use manifest_dir_macros::directory_relative_path;
use mint::Vector3;
use stardust_xr_fusion::{
	client::{Client, FrameInfo, RootHandler},
	core::values::Transform,
	drawable::{Model, ResourceID},
	fields::SphereField,
	node::NodeError,
};
use stardust_xr_molecules::{GrabData, Grabbable};

lazy_static! {
	static ref ICON_RESOURCE: ResourceID = ResourceID::new_namespaced("molecules", "urchin");
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped_root = client.wrap_root(GrabbableDemo::new(&client)?)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	}
	Ok(())
}

struct GrabbableDemo {
	grabbable: Grabbable,
	field: SphereField,
	model: Model,
}
impl GrabbableDemo {
	fn new(client: &Client) -> Result<Self, NodeError> {
		let field = SphereField::create(client.get_root(), mint::Vector3::from([0.0; 3]), 0.1)?;
		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::default(),
			&field,
			GrabData::default(),
		)?;
		let model = Model::create(
			client.get_root(),
			Transform::from_scale(Vector3::from([0.1; 3])),
			&*ICON_RESOURCE,
		)?;
		field.set_spatial_parent(grabbable.content_parent())?;

		Ok(GrabbableDemo {
			grabbable,
			field,
			model,
		})
	}
}
impl RootHandler for GrabbableDemo {
	fn frame(&mut self, info: FrameInfo) {
		self.grabbable.update(&info).unwrap();
		self.model
			.model_part("Icosphere.001")
			.unwrap()
			.set_spatial_parent(self.grabbable.content_parent())
			.unwrap();
	}
}
