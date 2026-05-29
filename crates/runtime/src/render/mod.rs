pub mod backend;
pub mod hardware;
pub mod scene_frame_presenter;
pub mod software;

pub use backend::RenderBackend;
pub use hardware::WgpuBackend;
pub use scene_frame_presenter::RuntimeSceneFramePresenter;
pub use software::{BuiltinSoftwareDrawer, SoftwareBackend, SoftwareDrawStrategy};
