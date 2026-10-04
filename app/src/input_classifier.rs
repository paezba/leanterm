use std::sync::Arc;

use input_classifier::{HeuristicClassifier, InputClassifier};
#[cfg(any(
    feature = "nld_classifier_v1",
    feature = "nld_classifier_v2",
    feature = "nld_classifier_v3"
))]
use input_classifier::{OnnxClassifier, OnnxModel};
use warpui::{Entity, ModelContext, SingletonEntity};

impl InputClassifierModel {

}

impl Entity for InputClassifierModel {
    type Event = ();
}

impl SingletonEntity for InputClassifierModel {}

pub struct InputClassifierModel {
    pub classifier: Arc<dyn InputClassifier>,
}
