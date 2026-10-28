use crate::{
    app::App,
    markdown::{toc::sticky_heading_route, width::rendered_span_width, LinkId, LinkSpan},
    theme::app_theme,
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use std::collections::HashMap;

use super::{CONTENT_HORIZONTAL_PADDING, SCROLLBAR_WIDTH};

pub(super) fn render_content_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let viewport_height = area.height as usize;
    let theme = app_theme();
    f.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.ui.content_bg)),
        area,
    );
    let content_area = inner_content_area(area);
    let scroll = app.scroll();
    let active_highlight_line = app.active_highlight_line();
    if let Some(line_idx) = active_highlight_line {
        let _ = app.refresh_highlighted_line_cache(line_idx);
    }

    let visible_end = (scroll + viewport_height).min(app.total());
    let mut visible_lines = app.visible_lines(scroll, visible_end).to_vec();

    if let Some(line_idx) = active_highlight_line {
        if (scroll..visible_end).contains(&line_idx) {
            if let Some((_, highlighted_line)) = app.highlighted_line_cache() {
                visible_lines[line_idx - scroll] = highlighted_line.clone();
            }
        }
    }

    let hovered_id = app
        .hovered_link
        .and_then(|(line, index)| app.link_span(line, index))
        .map(|link| link.link_id);
    if let Some(id) = hovered_id {
        style_link_occurrence(
            &mut visible_lines,
            &app.link_spans_by_line,
            scroll,
            id,
            Style::default().fg(theme.markdown.link_hover),
        );
    }

    if let Some(id) = app.selected_link() {
        style_link_occurrence(
            &mut visible_lines,
            &app.link_spans_by_line,
            scroll,
            id,
            Style::default()
                .fg(theme.markdown.link_text)
                .bg(theme.ui.content_bg)
                .add_modifier(Modifier::REVERSED),
        );
    }

    if let Some(block_idx) = app.highlighted_code_block() {
        if let Some(block) = app.code_blocks.get(block_idx) {
            apply_code_block_highlight(
                &mut visible_lines,
                block,
                scroll,
                visible_end,
                &theme.markdown,
            );
        }
    }

    if app.is_line_number_visible() {
        let digit_width = app.line_number_total().max(1).to_string().len();
        let gutter_style = Style::default().fg(theme.markdown.code_gutter);
        let goto_target = app.goto_line_target();
        let highlight_bg = theme.markdown.search_highlight_bg;
        for (i, line) in visible_lines.iter_mut().enumerate() {
            let idx = scroll + i;
            let logical = app.line_number_at(idx);
            let is_first =
                logical > 0 && (idx == 0 || app.line_number_at(idx.saturating_sub(1)) != logical);
            let is_goto_target = goto_target == Some(idx);
            let num_part = if is_first {
                format!("{:>w$}", logical, w = digit_width)
            } else {
                format!("{:>w$}", "", w = digit_width)
            };
            if is_goto_target {
                let hl = gutter_style.bg(highlight_bg);
                line.spans.insert(0, Span::styled("│ ", hl));
                line.spans
                    .insert(0, Span::styled(num_part, hl.fg(Color::White)));
                for span in &mut line.spans[2..] {
                    span.style = span.style.bg(highlight_bg);
                }
            } else {
                let gutter = format!("{num_part}│ ");
                line.spans.insert(0, Span::styled(gutter, gutter_style));
            }
        }
    }

    app.refresh_screen_geometry(&visible_lines, content_area.width, content_area.height);
    let sticky_route = sticky_heading_route(app.toc_entries(), scroll);

    let sticky_rows = if sticky_route.is_empty() {
        0
    } else {
        usize::from(content_area.height.min(2))
    };

    app.set_sticky_hidden_rows(sticky_rows);

    f.render_widget(
        Paragraph::new(visible_lines)
            .style(Style::default().bg(theme.ui.content_bg))
            .wrap(Wrap { trim: false }),
        content_area,
    );

    let (mouse_col, mouse_row) = app.mouse_position;

    if !sticky_route.is_empty() && content_area.height > 0 {
        let toc = app.toc_entries();
        let active_idx = *sticky_route.last().expect("non-empty sticky route");
        let ellipsis_style = Style::default()
            .fg(theme.markdown.heading_underline)
            .bg(theme.ui.content_bg);

        let heading_style = |level: u8| {
            let heading_color = match level {
                1 => theme.markdown.heading_1,
                2 => theme.markdown.heading_2,
                3 => theme.markdown.heading_3,
                4 => theme.markdown.heading_4,
                _ => theme.markdown.heading_other,
            };

            Style::default()
                .fg(heading_color)
                .bg(theme.ui.content_bg)
                .add_modifier(Modifier::BOLD)
        };

        let make_label = |idx: usize| toc[idx].title.clone();

        let ancestor_count = sticky_route.len().saturating_sub(1);

        // Fila superior: ruta de ancestros compacta.
        let route_labels: Vec<Span<'static>> = sticky_route
            .iter()
            .take(sticky_route.len().saturating_sub(1))
            .filter_map(|&idx| {
                toc.get(idx)?;

                let position = sticky_route
                    .iter()
                    .take(ancestor_count)
                    .position(|&ancestor_idx| ancestor_idx == idx)
                    .unwrap_or(0);

                let progress = if ancestor_count <= 1 {
                    1.0
                } else {
                    position as f32 / (ancestor_count - 1) as f32
                };

                let shade = (165.0 - progress * 95.0) as u8;
                let ancestor_style = Style::default()
                    .fg(ratatui::style::Color::Rgb(shade, shade, shade))
                    .bg(theme.ui.content_bg);

                Some(Span::styled(
                    format!(" {} ", make_label(idx)),
                    ancestor_style,
                ))
            })
            .collect();

        let route_spans =
            compact_sticky_route(route_labels, content_area.width as usize, ellipsis_style);

        // Fila inferior: el encabezado activo siempre tiene prioridad.
        let active_entry = &toc[active_idx];
        let active_spans = truncate_sticky_spans(
            vec![Span::styled(
                format!(
                    " {} ",
                    if active_entry.number.is_empty() {
                        active_entry.title.clone()
                    } else {
                        format!("{} {}", active_entry.number, active_entry.title)
                    }
                ),
                heading_style(active_entry.level),
            )],
            content_area.width as usize,
            ellipsis_style,
        );

        // En esta primera etapa dibujamos las dos líneas cuando hay espacio.
        // La reserva real de filas y el mapeo de enlaces se ajustarán en la etapa 2.
        let rows = content_area.height.min(2);

        if rows >= 2 {
            let route_area = Rect {
                x: content_area.x,
                y: content_area.y,
                width: content_area.width,
                height: 1,
            };
            let heading_area = Rect {
                x: content_area.x,
                y: content_area.y + 1,
                width: content_area.width,
                height: 1,
            };

            let sticky_area = Rect {
                x: content_area.x,
                y: content_area.y,
                width: content_area.width,
                height: rows,
            };

            // Rellenar explícitamente las dos filas con el fondo opaco.
            let background = Style::default().bg(theme.ui.content_bg);
            let blank_row = Line::from(Span::styled(
                " ".repeat(sticky_area.width as usize),
                background,
            ));

            f.render_widget(
                Paragraph::new(vec![blank_row.clone(), blank_row])
                    .style(background)
                    .wrap(Wrap { trim: false }),
                sticky_area,
            );

            for (row_area, spans) in [(route_area, route_spans), (heading_area, active_spans)] {
                f.render_widget(
                    Paragraph::new(Line::from(spans))
                        .style(Style::default().bg(theme.ui.content_bg)),
                    row_area,
                );
            }
        } else {
            // En una terminal de una sola fila mostramos únicamente el título actual.
            let sticky_area = Rect {
                x: content_area.x,
                y: content_area.y,
                width: content_area.width,
                height: 1,
            };

            let background = Style::default().bg(theme.ui.content_bg);
            let blank_row = Line::from(Span::styled(
                " ".repeat(sticky_area.width as usize),
                background,
            ));

            f.render_widget(
                Paragraph::new(blank_row)
                    .style(background)
                    .wrap(Wrap { trim: false }),
                sticky_area,
            );

            f.render_widget(
                Paragraph::new(Line::from(active_spans))
                    .style(Style::default().bg(theme.ui.content_bg)),
                sticky_area,
            );
        }
    }

    let sb_x = area
        .x
        .saturating_add(area.width)
        .saturating_sub(SCROLLBAR_WIDTH);
    let on_sb_column = mouse_col >= sb_x
        && mouse_col < sb_x + SCROLLBAR_WIDTH
        && mouse_row >= area.y
        && mouse_row < area.y + area.height;

    let max_scroll = app.max_scroll();
    let track_len = area.height as usize;
    let mouse_on_thumb = on_sb_column && track_len > 0 && max_scroll > 0 && {
        let thumb_size = (track_len * track_len / max_scroll).max(1).min(track_len);
        let max_offset = track_len.saturating_sub(thumb_size);
        let thumb_offset = app.scroll() * max_offset / max_scroll;
        let thumb_top = area.y as usize + thumb_offset;
        let thumb_bottom = thumb_top + thumb_size;
        let row = mouse_row as usize;
        row >= thumb_top && row < thumb_bottom
    };

    let mut scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("█");
    if mouse_on_thumb || app.scrollbar_dragging {
        scrollbar = scrollbar.thumb_style(Style::default().fg(theme.ui.scrollbar_hover));
    }

    let mut scrollbar_state = ScrollbarState::new(max_scroll).position(app.scroll());
    f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

pub(crate) fn inner_content_area(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(CONTENT_HORIZONTAL_PADDING),
        y: area.y,
        width: area
            .width
            .saturating_sub(CONTENT_HORIZONTAL_PADDING.saturating_mul(2))
            .saturating_sub(SCROLLBAR_WIDTH),
        height: area.height,
    }
}

fn apply_code_block_highlight(
    visible_lines: &mut [ratatui::text::Line<'static>],
    block: &crate::markdown::CodeBlockInfo,
    scroll: usize,
    visible_end: usize,
    theme: &crate::theme::MarkdownTheme,
) {
    let start = block.rendered_start.max(scroll);
    let end = block.rendered_end.min(visible_end.saturating_sub(1));
    if start > end {
        return;
    }
    let footer_line = block.rendered_end.saturating_sub(1);
    let focus = theme.link_hover;
    for line_idx in start..=end {
        let vis_idx = line_idx - scroll;
        let Some(line) = visible_lines.get_mut(vis_idx) else {
            continue;
        };
        let target = if line_idx == block.rendered_start || line_idx == footer_line {
            theme.code_frame
        } else {
            theme.code_gutter
        };
        for span in &mut line.spans {
            if span.style.fg == Some(target) {
                span.style = span.style.fg(focus);
            }
        }
    }
}

fn style_link_occurrence(
    lines: &mut [ratatui::text::Line<'static>],
    spans_by_line: &HashMap<usize, Vec<LinkSpan>>,
    first_line: usize,
    id: LinkId,
    style: Style,
) {
    for (offset, line) in lines.iter_mut().enumerate() {
        if let Some(ranges) = spans_by_line.get(&(first_line + offset)) {
            for link in ranges.iter().filter(|link| link.link_id == id) {
                apply_link_style(line, link.start_col, link.end_col, style);
            }
        }
    }
}

fn apply_link_style(
    line: &mut ratatui::text::Line<'static>,
    start_col: usize,
    end_col: usize,
    style: Style,
) {
    let mut col = 0usize;
    for span in &mut line.spans {
        let w = rendered_span_width(span);
        let span_end = col + w;
        if span_end > start_col && col < end_col {
            span.style = span.style.patch(style);
        }
        col = span_end;
    }
}

fn compact_sticky_route(
    labels: Vec<Span<'static>>,
    max_width: usize,
    ellipsis_style: Style,
) -> Vec<Span<'static>> {
    if max_width == 0 || labels.is_empty() {
        return Vec::new();
    }

    let width_of =
        |spans: &[Span<'static>]| -> usize { spans.iter().map(rendered_span_width).sum() };

    let separator = || Span::styled(" › ", ellipsis_style);
    let ellipsis = || Span::styled("…", ellipsis_style);

    let fixed_trim = labels.len() > 3;
    let first = labels[0].clone();

    // Para rutas largas: raíz + elipsis + los dos niveles más recientes.
    let tail_start = labels.len().saturating_sub(2);
    let tail_source = if fixed_trim {
        &labels[tail_start..]
    } else {
        &labels[1..]
    };

    let mut full_route = vec![first.clone()];

    if fixed_trim {
        full_route.push(ellipsis());
    } else if !tail_source.is_empty() {
        full_route.push(separator());
    }

    for (index, label) in tail_source.iter().enumerate() {
        if index > 0 || fixed_trim {
            full_route.push(separator());
        }
        full_route.push(label.clone());
    }

    if width_of(&full_route) <= max_width {
        return full_route;
    }

    if fixed_trim {
        // Mantener la raíz y la elipsis; añadir primero el nivel más cercano.
        let mut best = vec![first.clone(), ellipsis()];

        if width_of(&best) > max_width {
            return truncate_sticky_spans(vec![first], max_width, ellipsis_style);
        }

        let mut tail = Vec::new();

        for label in tail_source.iter().rev() {
            let mut candidate_tail = vec![label.clone()];
            candidate_tail.extend(tail.iter().cloned());

            let mut candidate = vec![first.clone(), ellipsis(), separator()];

            for (index, item) in candidate_tail.iter().enumerate() {
                if index > 0 {
                    candidate.push(separator());
                }
                candidate.push(item.clone());
            }

            if width_of(&candidate) > max_width {
                break;
            }

            tail = candidate_tail;
            best = candidate;
        }

        return best;
    }

    // Para rutas cortas, conservar el comportamiento de compactación horizontal.
    let mut best = vec![first.clone(), separator(), ellipsis()];

    if width_of(&best) > max_width {
        return truncate_sticky_spans(vec![first], max_width, ellipsis_style);
    }

    let mut tail = Vec::new();

    for label in labels.iter().skip(1).rev() {
        let mut candidate_tail = vec![label.clone()];
        candidate_tail.extend(tail.iter().cloned());

        let mut candidate = vec![first.clone(), separator(), ellipsis()];

        for item in &candidate_tail {
            candidate.push(separator());
            candidate.push(item.clone());
        }

        if width_of(&candidate) > max_width {
            break;
        }

        tail = candidate_tail;
        best = candidate;
    }

    best
}

fn truncate_sticky_spans(
    spans: Vec<Span<'static>>,
    max_width: usize,
    ellipsis_style: Style,
) -> Vec<Span<'static>> {
    let total_width: usize = spans.iter().map(rendered_span_width).sum();

    if total_width <= max_width {
        return spans;
    }

    if max_width == 0 {
        return Vec::new();
    }

    // Reservar una columna para la elipsis.
    let content_width = max_width - 1;
    let mut result = Vec::new();
    let mut used_width = 0;

    for span in spans {
        if used_width >= content_width {
            break;
        }

        let span_width = rendered_span_width(&span);

        if used_width + span_width <= content_width {
            used_width += span_width;
            result.push(span);
            continue;
        }

        let mut text = String::new();

        for ch in span.content.chars() {
            let candidate = Span::styled(ch.to_string(), span.style);
            let char_width = rendered_span_width(&candidate);

            if used_width + char_width > content_width {
                break;
            }

            text.push(ch);
            used_width += char_width;
        }

        if !text.is_empty() {
            result.push(Span::styled(text, span.style));
        }

        break;
    }

    result.push(Span::styled("…", ellipsis_style));
    result
}

pub(super) fn render_status_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let pct = app.scroll_percent();
    let bar_bg = super::status::status_bar_bg();
    app.refresh_status_cache(pct);

    f.render_widget(
        Paragraph::new(vec![app.status_line().clone()]).style(Style::default().bg(bar_bg)),
        area,
    );
}

#[cfg(test)]
mod sticky_route_tests {
    use super::*;

    fn label(text: &str) -> Span<'static> {
        Span::raw(text.to_owned())
    }

    fn plain_text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn sticky_route_keeps_short_routes_intact() {
        let route = compact_sticky_route(vec![label(" A "), label(" B ")], 20, Style::default());

        assert_eq!(plain_text(&route), " A  ›  B ");
    }

    #[test]
    fn sticky_route_keeps_first_and_nearest_ancestors() {
        let route = compact_sticky_route(
            vec![label(" A "), label(" B "), label(" C "), label(" D ")],
            19,
            Style::default(),
        );

        assert_eq!(plain_text(&route), " A … ›  C  ›  D ");
        assert!(route.iter().map(rendered_span_width).sum::<usize>() <= 19);
    }

    #[test]
    fn sticky_route_respects_narrow_widths() {
        for width in 0..=8 {
            let route = compact_sticky_route(
                vec![label(" Ancestor "), label(" Current ")],
                width,
                Style::default(),
            );

            assert!(
                route.iter().map(rendered_span_width).sum::<usize>() <= width,
                "route exceeds width {width}"
            );
        }
    }

    #[test]
    fn sticky_route_truncates_when_only_one_label_is_available() {
        let route = compact_sticky_route(vec![label(" A very long heading ")], 8, Style::default());

        assert_eq!(plain_text(&route), " A very…");
        assert!(route.iter().map(rendered_span_width).sum::<usize>() <= 8);
    }

    #[test]
    fn sticky_heading_truncation_respects_available_width() {
        let spans = vec![label(" A very long heading ")];

        let truncated = truncate_sticky_spans(spans, 8, Style::default());

        assert!(truncated.iter().map(rendered_span_width).sum::<usize>() <= 8);

        assert!(plain_text(&truncated).ends_with('…'));
    }
}
