use crate::consts::{BOUNDS_ROTATE_THRESHOLD, BOUNDS_SELECT_THRESHOLD, SELECTION_DRAG_ANGLE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::transformation::OriginalTransforms;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapTypeConfiguration;

use graphene_core::renderer::Quad;

use glam::{DAffine2, DVec2};
use graphene_std::renderer::Rect;

use super::snapping::{self, SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnappedPoint};

pub struct SizeSnapData<'a> {
	pub manager: &'a mut SnapManager,
	pub points: &'a mut Vec<SnapCandidatePoint>,
	pub snap_data: SnapData<'a>,
}

/// Contains the edges that are being dragged along with the original bounds.
#[derive(Clone, Debug, Default)]
pub struct SelectedEdges {
	pub bounds: [DVec2; 2],
	pub top: bool,
	pub bottom: bool,
	pub left: bool,
	pub right: bool,
	// Aspect ratio in the form of width/height, so x:1 = width:height
	aspect_ratio: f64,
}

impl SelectedEdges {
	pub fn new(top: bool, bottom: bool, left: bool, right: bool, bounds: [DVec2; 2]) -> Self {
		let size = (bounds[0] - bounds[1]).abs();
		let aspect_ratio = size.x / size.y;
		Self {
			top,
			bottom,
			left,
			right,
			bounds,
			aspect_ratio,
		}
	}

	/// Calculate the pivot for the operation (the opposite point to the edge dragged)
	pub fn calculate_pivot(&self) -> DVec2 {
		self.pivot_from_bounds(self.bounds[0], self.bounds[1])
	}

	fn pivot_from_bounds(&self, min: DVec2, max: DVec2) -> DVec2 {
		let x = if self.left {
			max.x
		} else if self.right {
			min.x
		} else {
			(min.x + max.x) / 2.
		};

		let y = if self.top {
			max.y
		} else if self.bottom {
			min.y
		} else {
			(min.y + max.y) / 2.
		};

		DVec2::new(x, y)
	}

	/// Computes the new bounds with the given mouse move and modifier keys
	pub fn new_size(&self, mouse: DVec2, transform: DAffine2, center_around: Option<DVec2>, constrain: bool, snap: Option<SizeSnapData>) -> (DVec2, DVec2) {
		let mouse = transform.inverse().transform_point2(mouse);

		let mut min = self.bounds[0];
		let mut max = self.bounds[1];
		if self.top {
			min.y = mouse.y;
		} else if self.bottom {
			max.y = mouse.y;
		}
		if self.left {
			min.x = mouse.x;
		} else if self.right {
			max.x = mouse.x;
		}

		let mut pivot = self.pivot_from_bounds(min, max);
		if let Some(center_around) = center_around {
			let center_around = transform.inverse().transform_point2(center_around);
			if self.top {
				pivot.y = center_around.y;
				max.y = center_around.y * 2. - min.y;
			} else if self.bottom {
				pivot.y = center_around.y;
				min.y = center_around.y * 2. - max.y;
			}
			if self.left {
				pivot.x = center_around.x;
				max.x = center_around.x * 2. - min.x;
			} else if self.right {
				pivot.x = center_around.x;
				min.x = center_around.x * 2. - max.x;
			}
		}

		if constrain {
			let size = max - min;
			let min_pivot = (pivot - min) / size;
			let new_size = match ((self.top || self.bottom), (self.left || self.right)) {
				(true, true) => DVec2::new(size.x, size.x / self.aspect_ratio).abs().max(DVec2::new(size.y * self.aspect_ratio, size.y).abs()) * size.signum(),
				(true, false) => DVec2::new(size.y * self.aspect_ratio, size.y),
				(false, true) => DVec2::new(size.x, size.x / self.aspect_ratio),
				_ => size,
			};
			let delta_size = new_size - size;
			min -= delta_size * min_pivot;
			max = min + new_size;
		}

		if let Some(SizeSnapData { manager, points, snap_data }) = snap {
			let view_to_doc = snap_data.document.metadata().document_to_viewport.inverse();
			let bounds_to_doc = view_to_doc * transform;
			let mut best_snap = SnappedPoint::infinite_snap(pivot);
			let mut best_scale_factor = DVec2::ONE;
			let tolerance = snapping::snap_tolerance(snap_data.document);

			let bbox = Some(Rect::from_box((bounds_to_doc * Quad::from_box([min, max])).bounding_box()));
			for (index, point) in points.iter_mut().enumerate() {
				let config = SnapTypeConfiguration {
					bbox,
					use_existing_candidates: index != 0,
					..Default::default()
				};

				let old_position = point.document_point;
				let bounds_space = bounds_to_doc.inverse().transform_point2(point.document_point);
				let normalized = (bounds_space - self.bounds[0]) / (self.bounds[1] - self.bounds[0]);
				let updated = normalized * (max - min) + min;
				point.document_point = bounds_to_doc.transform_point2(updated);
				let mut snapped = if constrain {
					let constraint = SnapConstraint::Line {
						origin: point.document_point,
						direction: (point.document_point - bounds_to_doc.transform_point2(pivot)).normalize_or_zero(),
					};
					manager.constrained_snap(&snap_data, point, constraint, config)
				} else if !(self.top || self.bottom) || !(self.left || self.right) {
					let axis = if !(self.top || self.bottom) { DVec2::X } else { DVec2::Y };
					let constraint = SnapConstraint::Line {
						origin: point.document_point,
						direction: bounds_to_doc.transform_vector2(axis),
					};
					manager.constrained_snap(&snap_data, point, constraint, config)
				} else {
					manager.free_snap(&snap_data, point, config)
				};
				point.document_point = old_position;

				if !snapped.is_snapped() {
					continue;
				}
				let snapped_bounds = bounds_to_doc.inverse().transform_point2(snapped.snapped_point_document);

				let mut scale_factor = (snapped_bounds - pivot) / (updated - pivot);
				if !(self.left || self.right || constrain) {
					scale_factor.x = 1.
				}
				if !(self.top || self.bottom || constrain) {
					scale_factor.y = 1.
				}

				snapped.distance = bounds_to_doc.transform_vector2((max - min) * (scale_factor - DVec2::ONE)).length();
				if snapped.distance > tolerance || !snapped.distance.is_finite() {
					continue;
				}
				if best_snap.other_snap_better(&snapped) {
					best_snap = snapped;
					best_scale_factor = scale_factor;
				}
			}
			manager.update_indicator(best_snap);

			min = pivot - (pivot - min) * best_scale_factor;
			max = pivot - (pivot - max) * best_scale_factor;
		}

		(min, max - min)
	}

	/// Calculates the required scaling to resize the bounding box
	pub fn bounds_to_scale_transform(&self, position: DVec2, size: DVec2) -> (DAffine2, DVec2) {
		let old_size = self.bounds[1] - self.bounds[0];
		let mut enlargement_factor = size / old_size;
		if !enlargement_factor.x.is_finite() || old_size.x.abs() < f64::EPSILON * 1000. {
			enlargement_factor.x = 1.;
		}
		if !enlargement_factor.y.is_finite() || old_size.y.abs() < f64::EPSILON * 1000. {
			enlargement_factor.y = 1.;
		}
		let mut pivot = (self.bounds[0] * enlargement_factor - position) / (enlargement_factor - DVec2::ONE);
		if !pivot.x.is_finite() {
			pivot.x = 0.;
		}
		if !pivot.y.is_finite() {
			pivot.y = 0.;
		}
		(DAffine2::from_scale(enlargement_factor), pivot)
	}
}

/// Aligns the mouse position to the closest axis
pub fn axis_align_drag(axis_align: bool, position: DVec2, start: DVec2) -> DVec2 {
	if axis_align {
		let mouse_position = position - start;
		let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
		let angle = -mouse_position.angle_to(DVec2::X);
		let snapped_angle = (angle / snap_resolution).round() * snap_resolution;
		if snapped_angle.is_finite() {
			start + DVec2::new(snapped_angle.cos(), snapped_angle.sin()) * mouse_position.length()
		} else {
			start
		}
	} else {
		position
	}
}

/// Snaps a dragging event from the artboard or select tool
pub fn snap_drag(start: DVec2, current: DVec2, axis_align: bool, snap_data: SnapData, snap_manager: &mut SnapManager, candidates: &[SnapCandidatePoint]) -> DVec2 {
	let mouse_position = axis_align_drag(axis_align, snap_data.input.mouse.position, start);
	let document = snap_data.document;
	let total_mouse_delta_document = document.metadata().document_to_viewport.inverse().transform_vector2(mouse_position - start);
	let mouse_delta_document = document.metadata().document_to_viewport.inverse().transform_vector2(mouse_position - current);
	let mut offset = mouse_delta_document;
	let mut best_snap = SnappedPoint::infinite_snap(document.metadata().document_to_viewport.inverse().transform_point2(mouse_position));

	let bbox = Rect::point_iter(candidates.iter().map(|candidate| candidate.document_point + total_mouse_delta_document));

	for (index, point) in candidates.iter().enumerate() {
		let config = SnapTypeConfiguration {
			bbox,
			accept_distribution: true,
			use_existing_candidates: index != 0,
			..Default::default()
		};

		let mut point = point.clone();
		point.document_point += total_mouse_delta_document;

		let snapped = if axis_align {
			let constraint = SnapConstraint::Line {
				origin: point.document_point,
				direction: total_mouse_delta_document.try_normalize().unwrap_or(DVec2::X),
			};
			snap_manager.constrained_snap(&snap_data, &point, constraint, config)
		} else {
			snap_manager.free_snap(&snap_data, &point, config)
		};

		if best_snap.other_snap_better(&snapped) {
			offset = snapped.snapped_point_document - point.document_point + mouse_delta_document;
			best_snap = snapped;
		}
	}

	snap_manager.update_indicator(best_snap);

	document.metadata().document_to_viewport.transform_vector2(offset)
}

/// Contains info on the overlays for the bounding box and transform handles
#[derive(Clone, Debug, Default)]
pub struct BoundingBoxManager {
	pub bounds: [DVec2; 2],
	pub transform: DAffine2,
	pub original_bound_transform: DAffine2,
	pub selected_edges: Option<SelectedEdges>,
	pub original_transforms: OriginalTransforms,
	pub opposite_pivot: DVec2,
	pub center_of_transformation: DVec2,
}

impl BoundingBoxManager {
	/// Calculates the transformed handle positions based on the bounding box and the transform
	pub fn evaluate_transform_handle_positions(&self) -> [DVec2; 8] {
		let (left, top): (f64, f64) = self.bounds[0].into();
		let (right, bottom): (f64, f64) = self.bounds[1].into();
		[
			self.transform.transform_point2(DVec2::new(left, top)),
			self.transform.transform_point2(DVec2::new(left, (top + bottom) / 2.)),
			self.transform.transform_point2(DVec2::new(left, bottom)),
			self.transform.transform_point2(DVec2::new((left + right) / 2., top)),
			self.transform.transform_point2(DVec2::new((left + right) / 2., bottom)),
			self.transform.transform_point2(DVec2::new(right, top)),
			self.transform.transform_point2(DVec2::new(right, (top + bottom) / 2.)),
			self.transform.transform_point2(DVec2::new(right, bottom)),
		]
	}

	/// Update the position of the bounding box and transform handles
	pub fn render_overlays(&mut self, overlay_context: &mut OverlayContext) {
		overlay_context.quad(self.transform * Quad::from_box(self.bounds), None);

		for position in self.evaluate_transform_handle_positions() {
			overlay_context.square(position, Some(6.), None, None);
		}
	}

	/// Compute the threshold in viewport space. This only works with affine transforms as it assumes lines remain parallel.
	fn compute_viewport_threshold(&self, scalar: f64) -> [f64; 2] {
		let inverse = self.transform.inverse();

		let viewport_x = self.transform.transform_vector2(DVec2::X).normalize_or_zero() * scalar;
		let viewport_y = self.transform.transform_vector2(DVec2::Y).normalize_or_zero() * scalar;
		let threshold_x = inverse.transform_vector2(viewport_x).length();
		let threshold_y = inverse.transform_vector2(viewport_y).length();
		[threshold_x, threshold_y]
	}

	/// Check if the user has selected the edge for dragging (returns which edge in order top, bottom, left, right)
	pub fn check_selected_edges(&self, cursor: DVec2) -> Option<(bool, bool, bool, bool)> {
		let cursor = self.transform.inverse().transform_point2(cursor);

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);
		let [threshold_x, threshold_y] = self.compute_viewport_threshold(BOUNDS_SELECT_THRESHOLD);

		if min.x - cursor.x < threshold_x && min.y - cursor.y < threshold_y && cursor.x - max.x < threshold_x && cursor.y - max.y < threshold_y {
			let mut top = (cursor.y - min.y).abs() < threshold_y;
			let mut bottom = (max.y - cursor.y).abs() < threshold_y;
			let mut left = (cursor.x - min.x).abs() < threshold_x;
			let mut right = (max.x - cursor.x).abs() < threshold_x;

			// Prioritise single axis transformations on very small bounds
			if cursor.y - min.y + max.y - cursor.y < threshold_y * 2. && (left || right) {
				top = false;
				bottom = false;
			}
			if cursor.x - min.x + max.x - cursor.x < threshold_x * 2. && (top || bottom) {
				left = false;
				right = false;
			}

			// On bounds with no width/height, disallow transformation in the relevant axis
			if (max.x - min.x) < f64::EPSILON * 1000. {
				left = false;
				right = false;
			}
			if (max.y - min.y) < f64::EPSILON * 1000. {
				top = false;
				bottom = false;
			}

			if top || bottom || left || right {
				return Some((top, bottom, left, right));
			}
		}

		None
	}

	/// Check if the user is rotating with the bounds
	pub fn check_rotate(&self, cursor: DVec2) -> bool {
		let cursor = self.transform.inverse().transform_point2(cursor);
		let [threshold_x, threshold_y] = self.compute_viewport_threshold(BOUNDS_ROTATE_THRESHOLD);

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);

		let outside_bounds = (min.x > cursor.x || cursor.x > max.x) || (min.y > cursor.y || cursor.y > max.y);
		let inside_extended_bounds = min.x - cursor.x < threshold_x && min.y - cursor.y < threshold_y && cursor.x - max.x < threshold_x && cursor.y - max.y < threshold_y;

		outside_bounds & inside_extended_bounds
	}

	/// Gets the required mouse cursor to show resizing bounds or optionally rotation
	pub fn get_cursor(&self, input: &InputPreprocessorMessageHandler, rotate: bool) -> MouseCursorIcon {
		if let Some(directions) = self.check_selected_edges(input.mouse.position) {
			match directions {
				(true, _, false, false) | (_, true, false, false) => MouseCursorIcon::NSResize,
				(false, false, true, _) | (false, false, _, true) => MouseCursorIcon::EWResize,
				(true, _, true, _) | (_, true, _, true) => MouseCursorIcon::NWSEResize,
				(true, _, _, true) | (_, true, true, _) => MouseCursorIcon::NESWResize,
				_ => MouseCursorIcon::Default,
			}
		} else if rotate && self.check_rotate(input.mouse.position) {
			MouseCursorIcon::Rotate
		} else {
			MouseCursorIcon::Default
		}
	}
}
