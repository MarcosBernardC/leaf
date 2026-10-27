const MAX_TOC_DISPLAY_LEVELS: usize = 5;
const MAX_MARKDOWN_HEADING_LEVELS: usize = 6;

#[derive(Clone)]
pub(crate) struct TocEntry {
    pub(crate) level: u8,
    pub(crate) title: String,
    pub(crate) number: String,
    pub(crate) line: usize,
}

pub(crate) struct TocLevels {
    pub(crate) root: u8,
    pub(crate) sub: Option<u8>,
    visible: Vec<u8>,
}

impl TocLevels {
    pub(crate) fn display_level(&self, level: u8) -> Option<u8> {
        self.visible
            .iter()
            .position(|&candidate| candidate == level)
            .map(|index| (index + 1) as u8)
    }
}

fn distinct_levels(toc: &[TocEntry]) -> Vec<u8> {
    let mut levels: Vec<u8> = toc.iter().map(|e| e.level).collect();
    levels.sort_unstable();
    levels.dedup();
    levels
}

pub(crate) fn toc_levels(toc: &[TocEntry]) -> Option<TocLevels> {
    let levels = distinct_levels(toc);
    let &top = levels.first()?;
    let top_unique = toc.iter().filter(|e| e.level == top).count() == 1;

    // Preserve the existing behavior of promoting a unique title heading.
    let root_idx = if top_unique && levels.len() >= 2 {
        1
    } else {
        0
    };

    let visible: Vec<u8> = levels
        .into_iter()
        .skip(root_idx)
        .take(MAX_TOC_DISPLAY_LEVELS)
        .collect();

    let root = *visible.first()?;
    let sub = visible.get(1).copied();

    Some(TocLevels { root, sub, visible })
}

pub(crate) fn normalize_toc(mut toc: Vec<TocEntry>) -> Vec<TocEntry> {
    let levels = distinct_levels(&toc);
    let Some(&max_keep) = levels
        .get(MAX_MARKDOWN_HEADING_LEVELS - 1)
        .or_else(|| levels.last())
    else {
        return toc;
    };

    toc.retain(|entry| entry.level <= max_keep);
    toc
}

pub(crate) fn sticky_heading_route(toc: &[TocEntry], scroll: usize) -> Vec<usize> {
    let mut route: Vec<usize> = Vec::new();

    for (idx, entry) in toc.iter().enumerate() {
        // No mostrar sticky mientras el encabezado siga en la primera fila.
        if entry.line >= scroll {
            break;
        }

        // Un encabezado nuevo reemplaza a los de su mismo nivel
        // y a todos sus descendientes anteriores.
        route.retain(|&ancestor_idx| toc[ancestor_idx].level < entry.level);
        route.push(idx);
    }

    route
}
