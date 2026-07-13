use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::format::{GroupKind, GspFile, IndexedPathRecord, ObjectGroup, decode_indexed_path};
use crate::runtime::functions::{
    FunctionExpr, FunctionExprParseError, try_decode_function_expr,
    try_decode_standalone_function_expr,
};
use crate::runtime::payload_consts::{RECORD_INDEXED_PATH_A, RECORD_INDEXED_PATH_B};

use super::decode::IndexedPathDecodeError;

pub(crate) struct SceneContext<'a> {
    pub(crate) file: &'a GspFile,
    pub(crate) groups: &'a [ObjectGroup],
    indexed_paths: Vec<Result<Option<IndexedPathRecord>, IndexedPathDecodeError>>,
    refs_to: Vec<Vec<usize>>,
    groups_by_kind: BTreeMap<GroupKind, Vec<usize>>,
    function_exprs: RefCell<Vec<Option<Result<FunctionExpr, FunctionExprParseError>>>>,
    standalone_function_exprs: RefCell<Vec<Option<Result<FunctionExpr, FunctionExprParseError>>>>,
}

impl<'a> SceneContext<'a> {
    pub(crate) fn new(file: &'a GspFile, groups: &'a [ObjectGroup]) -> Self {
        let indexed_paths = groups
            .iter()
            .map(|group| decode_group_indexed_path(file, group))
            .collect::<Vec<_>>();
        let mut refs_to = vec![Vec::new(); groups.len()];
        for (group_index, path) in indexed_paths.iter().enumerate() {
            let Ok(Some(path)) = path else {
                continue;
            };
            for ordinal in &path.refs {
                if let Some(index) = ordinal.checked_sub(1)
                    && let Some(referrers) = refs_to.get_mut(index)
                {
                    referrers.push(group_index);
                }
            }
        }
        let mut groups_by_kind = BTreeMap::<GroupKind, Vec<usize>>::new();
        for (index, group) in groups.iter().enumerate() {
            groups_by_kind
                .entry(group.header.kind())
                .or_default()
                .push(index);
        }
        Self {
            file,
            groups,
            indexed_paths,
            refs_to,
            groups_by_kind,
            function_exprs: RefCell::new(vec![None; groups.len()]),
            standalone_function_exprs: RefCell::new(vec![None; groups.len()]),
        }
    }

    pub(crate) fn group(&self, index: usize) -> Option<&'a ObjectGroup> {
        self.groups.get(index)
    }

    pub(crate) fn group_by_ordinal(&self, ordinal: usize) -> Option<&'a ObjectGroup> {
        self.groups.get(ordinal.checked_sub(1)?)
    }

    pub(crate) fn group_index_by_ordinal(&self, ordinal: usize) -> Option<usize> {
        let index = ordinal.checked_sub(1)?;
        self.groups.get(index)?;
        Some(index)
    }

    pub(crate) fn indexed_path(&self, group: &ObjectGroup) -> Option<&IndexedPathRecord> {
        match self
            .indexed_paths
            .get(group.ordinal.checked_sub(1)?)?
            .as_ref()
        {
            Ok(path) => path.as_ref(),
            Err(error) => panic!(
                "validated scene contains malformed indexed path in group #{}: {error}",
                group.ordinal
            ),
        }
    }

    pub(crate) fn path_ref_group_index(
        &self,
        path: &IndexedPathRecord,
        ref_index: usize,
    ) -> Option<usize> {
        self.group_index_by_ordinal(*path.refs.get(ref_index)?)
    }

    pub(crate) fn path_ref_group(
        &self,
        path: &IndexedPathRecord,
        ref_index: usize,
    ) -> Option<&'a ObjectGroup> {
        self.group(self.path_ref_group_index(path, ref_index)?)
    }

    pub(crate) fn referrers(&self, ordinal: usize) -> &[usize] {
        ordinal
            .checked_sub(1)
            .and_then(|index| self.refs_to.get(index))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn group_indices_by_kind(&self, kind: GroupKind) -> &[usize] {
        self.groups_by_kind
            .get(&kind)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn has_referrer_matching(
        &self,
        ordinal: usize,
        mut predicate: impl FnMut(&ObjectGroup, &IndexedPathRecord) -> bool,
    ) -> bool {
        self.referrers(ordinal).iter().any(|index| {
            let Some(group) = self.group(*index) else {
                return false;
            };
            let Some(path) = self.indexed_path(group) else {
                return false;
            };
            predicate(group, path)
        })
    }

    pub(crate) fn function_expr(
        &self,
        group: &ObjectGroup,
    ) -> Result<FunctionExpr, FunctionExprParseError> {
        let Some(index) = group.ordinal.checked_sub(1) else {
            return try_decode_function_expr(self.file, self.groups, group);
        };
        if let Some(cached) = self.function_exprs.borrow().get(index).cloned().flatten() {
            return cached;
        }
        let decoded = try_decode_function_expr(self.file, self.groups, group);
        if let Some(slot) = self.function_exprs.borrow_mut().get_mut(index) {
            *slot = Some(decoded.clone());
        }
        decoded
    }

    pub(crate) fn standalone_function_expr(
        &self,
        group: &ObjectGroup,
    ) -> Result<FunctionExpr, FunctionExprParseError> {
        let Some(index) = group.ordinal.checked_sub(1) else {
            return try_decode_standalone_function_expr(self.file, self.groups, group);
        };
        if let Some(cached) = self
            .standalone_function_exprs
            .borrow()
            .get(index)
            .cloned()
            .flatten()
        {
            return cached;
        }
        let decoded = try_decode_standalone_function_expr(self.file, self.groups, group);
        if let Some(slot) = self.standalone_function_exprs.borrow_mut().get_mut(index) {
            *slot = Some(decoded.clone());
        }
        decoded
    }
}

fn decode_group_indexed_path(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<IndexedPathRecord>, IndexedPathDecodeError> {
    let record = group.records.iter().find(|record| {
        matches!(
            record.record_type,
            RECORD_INDEXED_PATH_A | RECORD_INDEXED_PATH_B
        )
    });
    let Some(record) = record else {
        return Ok(None);
    };
    decode_indexed_path(record.record_type, record.payload(&file.data))
        .map(Some)
        .ok_or(IndexedPathDecodeError::MalformedPathRecord {
            record_type: record.record_type,
            offset: record.offset,
            length: record.length,
        })
}
