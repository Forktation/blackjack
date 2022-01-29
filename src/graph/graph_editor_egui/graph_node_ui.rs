use crate::color_hex_utils::*;
use crate::prelude::graph::*;

use egui::*;
use epaint::*;

pub type PortLocations = std::collections::HashMap<AnyParameterId, Pos2>;

pub enum DrawGraphNodeResponse {
    ConnectEventStarted(NodeId, AnyParameterId),
    ConnectEventEnded(AnyParameterId),
    SetActiveNode(NodeId),
    RunNodeSideEffect(NodeId),
    ClearActiveNode,
    DeleteNode(NodeId),
    DisconnectEvent(InputId),
}

pub struct GraphNodeWidget<'a> {
    pub position: &'a mut Pos2,
    pub graph: &'a mut Graph,
    pub port_locations: &'a mut PortLocations,
    pub node_id: NodeId,
    pub ongoing_drag: Option<(NodeId, AnyParameterId)>,
    pub active: bool,
    pub pan: egui::Vec2,
}

impl<'a> GraphNodeWidget<'a> {
    pub const MAX_NODE_SIZE: [f32; 2] = [200.0, 200.0];

    pub fn show(self, ui: &mut Ui) -> Option<DrawGraphNodeResponse> {
        let mut child_ui = ui.child_ui(
            Rect::from_min_size(*self.position + self.pan, Self::MAX_NODE_SIZE.into()),
            Layout::default(),
        );

        let node_resp = Self::show_graph_node(
            self.graph,
            self.node_id,
            &mut child_ui,
            self.port_locations,
            self.ongoing_drag,
            self.active,
        );

        let resp = ui.allocate_rect(child_ui.min_rect(), Sense::drag());
        *self.position += resp.drag_delta();

        node_resp
    }

    /// Draws this node. Also fills in the list of port locations with all of its ports.
    /// Returns a response showing whether a drag event was started.
    /// Parameters:
    /// - **ongoing_drag**: Is there a port drag event currently going on?
    fn show_graph_node(
        graph: &mut Graph,
        node_id: NodeId,
        ui: &mut Ui,
        port_locations: &mut PortLocations,
        ongoing_drag: Option<(NodeId, AnyParameterId)>,
        active: bool,
    ) -> Option<DrawGraphNodeResponse> {
        let margin = egui::vec2(15.0, 5.0);
        let _field_separation = 5.0;
        let mut response: Option<DrawGraphNodeResponse> = None;

        let background_color = color_from_hex("#3f3f3f").unwrap();
        let titlebar_color = background_color.linear_multiply(0.8);
        let text_color = color_from_hex("#fefefe").unwrap();

        ui.visuals_mut().widgets.noninteractive.fg_stroke = Stroke::new(2.0, text_color);

        // Preallocate shapes to paint below contents
        let background_shape = ui.painter().add(Shape::Noop);

        let outer_rect_bounds = ui.available_rect_before_wrap();
        let mut inner_rect = outer_rect_bounds.shrink2(margin);

        // Make sure we don't shrink to the negative:
        inner_rect.max.x = inner_rect.max.x.max(inner_rect.min.x);
        inner_rect.max.y = inner_rect.max.y.max(inner_rect.min.y);

        let mut child_ui = ui.child_ui(inner_rect, *ui.layout());
        let mut title_height = 0.0;

        let mut input_port_heights = vec![];
        let mut output_port_heights = vec![];

        child_ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add(Label::new(
                    RichText::new(&graph[node_id].label)
                        .text_style(TextStyle::Button)
                        .color(color_from_hex("#fefefe").unwrap()),
                ));
            });
            ui.add_space(margin.y);
            title_height = ui.min_size().y;

            // First pass: Draw the inner fields. Compute port heights
            let inputs = graph[node_id].inputs.clone();
            for (param_name, param) in inputs {
                let height_before = ui.min_rect().bottom();
                if graph.connection(param).is_some() {
                    ui.label(param_name);
                } else {
                    graph[param].value_widget(&param_name, ui);
                }
                let height_after = ui.min_rect().bottom();
                input_port_heights.push((height_before + height_after) / 2.0);
            }

            let outputs = graph[node_id].outputs.clone();
            for (param_name, _param) in outputs {
                let height_before = ui.min_rect().bottom();
                ui.label(&param_name);
                let height_after = ui.min_rect().bottom();
                output_port_heights.push((height_before + height_after) / 2.0);
            }

            // Button row
            ui.horizontal(|ui| {
                // Show 'Enable' button for nodes that output a mesh
                if graph[node_id].can_be_enabled(graph) {
                    ui.horizontal(|ui| {
                        if !active {
                            if ui.button("👁 Set active").clicked() {
                                response = Some(DrawGraphNodeResponse::SetActiveNode(node_id));
                            }
                        } else {
                            let button = egui::Button::new(
                                RichText::new("👁 Active").color(egui::Color32::BLACK),
                            )
                            .fill(egui::Color32::GOLD);
                            if ui.add(button).clicked() {
                                response = Some(DrawGraphNodeResponse::ClearActiveNode);
                            }
                        }
                    });
                }
                // Show 'Run' button for executable nodes
                if graph[node_id].is_executable() && ui.button("⛭ Run").clicked() {
                    response = Some(DrawGraphNodeResponse::RunNodeSideEffect(node_id));
                }
            })
        });

        // Second pass, iterate again to draw the ports. This happens outside
        // the child_ui because we want ports to overflow the node background.

        let outer_rect = child_ui.min_rect().expand2(margin);
        let port_left = outer_rect.left();
        let port_right = outer_rect.right();

        #[allow(clippy::too_many_arguments)]
        fn draw_port(
            ui: &mut Ui,
            graph: &Graph,
            node_id: NodeId,
            port_pos: Pos2,
            response: &mut Option<DrawGraphNodeResponse>,
            param_id: AnyParameterId,
            port_locations: &mut PortLocations,
            ongoing_drag: Option<(NodeId, AnyParameterId)>,
            is_connected_input: bool,
        ) {
            let port_type = graph.any_param_type(param_id).unwrap();

            let port_rect = Rect::from_center_size(port_pos, egui::vec2(10.0, 10.0));

            let sense = if ongoing_drag.is_some() {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            };

            let resp = ui.allocate_rect(port_rect, sense);
            let port_color = if resp.hovered() {
                Color32::WHITE
            } else {
                GraphNodeWidget::data_type_color(port_type)
            };
            ui.painter()
                .circle(port_rect.center(), 5.0, port_color, Stroke::none());

            if resp.drag_started() {
                if is_connected_input {
                    *response = Some(DrawGraphNodeResponse::DisconnectEvent(
                        param_id.assume_input(),
                    ));
                } else {
                    *response = Some(DrawGraphNodeResponse::ConnectEventStarted(
                        node_id, param_id,
                    ));
                }
            }

            if let Some((origin_node, origin_param)) = ongoing_drag {
                if origin_node != node_id {
                    // Don't allow self-loops
                    if graph.any_param_type(origin_param).unwrap() == port_type
                        && resp.hovered()
                        && ui.input().pointer.any_released()
                    {
                        *response = Some(DrawGraphNodeResponse::ConnectEventEnded(param_id));
                    }
                }
            }

            port_locations.insert(param_id, port_rect.center());
        }

        // Input ports
        for ((_, param), port_height) in graph[node_id]
            .inputs
            .iter()
            .zip(input_port_heights.into_iter())
        {
            let should_draw = match graph[*param].kind() {
                InputParamKind::ConnectionOnly => true,
                InputParamKind::ConstantOnly => false,
                InputParamKind::ConnectionOrConstant => true,
            };

            if should_draw {
                let pos_left = pos2(port_left, port_height);
                draw_port(
                    ui,
                    graph,
                    node_id,
                    pos_left,
                    &mut response,
                    AnyParameterId::Input(*param),
                    port_locations,
                    ongoing_drag,
                    graph.connection(*param).is_some(),
                );
            }
        }

        // Output ports
        for ((_, param), port_height) in graph[node_id]
            .outputs
            .iter()
            .zip(output_port_heights.into_iter())
        {
            let pos_right = pos2(port_right, port_height);
            draw_port(
                ui,
                graph,
                node_id,
                pos_right,
                &mut response,
                AnyParameterId::Output(*param),
                port_locations,
                ongoing_drag,
                false,
            );
        }

        // Draw the background shape.
        // NOTE: This code is a bit more involve than it needs to be because egui
        // does not support drawing rectangles with asymmetrical round corners.

        let shape = {
            let corner_radius = 4.0;

            let titlebar_height = title_height + margin.y;
            let titlebar_rect =
                Rect::from_min_size(outer_rect.min, vec2(outer_rect.width(), titlebar_height));
            let titlebar = Shape::Rect(RectShape {
                rect: titlebar_rect,
                corner_radius,
                fill: titlebar_color,
                stroke: Stroke::none(),
            });

            let body_rect = Rect::from_min_size(
                outer_rect.min + vec2(0.0, titlebar_height - corner_radius),
                vec2(outer_rect.width(), outer_rect.height() - titlebar_height),
            );
            let body = Shape::Rect(RectShape {
                rect: body_rect,
                corner_radius: 0.0,
                fill: background_color,
                stroke: Stroke::none(),
            });

            let bottom_body_rect = Rect::from_min_size(
                body_rect.min + vec2(0.0, body_rect.height() - titlebar_height * 0.5),
                vec2(outer_rect.width(), titlebar_height),
            );
            let bottom_body = Shape::Rect(RectShape {
                rect: bottom_body_rect,
                corner_radius,
                fill: background_color,
                stroke: Stroke::none(),
            });

            Shape::Vec(vec![titlebar, body, bottom_body])
        };

        ui.painter().set(background_shape, shape);
        ui.allocate_rect(outer_rect, Sense::hover());

        // Titlebar buttons
        if Self::close_button(ui, outer_rect).clicked() {
            response = Some(DrawGraphNodeResponse::DeleteNode(node_id));
        };

        response
    }

    fn close_button(ui: &mut Ui, node_rect: Rect) -> Response {
        // Measurements
        let margin = 8.0;
        let size = 10.0;
        let stroke_width = 2.0;
        let offs = margin + size / 2.0;

        let position = pos2(node_rect.right() - offs, node_rect.top() + offs);
        let rect = Rect::from_center_size(position, vec2(size, size));
        let resp = ui.allocate_rect(rect, Sense::click());

        let color = if resp.clicked() {
            color_from_hex("#ffffff").unwrap()
        } else if resp.hovered() {
            color_from_hex("#dddddd").unwrap()
        } else {
            color_from_hex("#aaaaaa").unwrap()
        };
        let stroke = Stroke {
            width: stroke_width,
            color,
        };

        ui.painter()
            .line_segment([rect.left_top(), rect.right_bottom()], stroke);
        ui.painter()
            .line_segment([rect.right_top(), rect.left_bottom()], stroke);

        resp
    }

    /// The port colors for all the data types
    fn data_type_color(data_type: DataType) -> egui::Color32 {
        match data_type {
            DataType::Mesh => color_from_hex("#266dd3").unwrap(),
            DataType::Vector => color_from_hex("#eecf6d").unwrap(),
            DataType::Scalar => color_from_hex("#eb9fef").unwrap(),
            DataType::Selection => color_from_hex("#4b7f52").unwrap(),
            DataType::Enum => color_from_hex("#ff0000").unwrap(), // Should never be in a port, so highlight in red
            DataType::NewFile => color_from_hex("#ff0000").unwrap(), // Should never be in a port, so highlight in red
        }
    }
}
