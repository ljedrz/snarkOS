// Copyright (C) 2019-2020 Aleo Systems Inc.
// This file is part of the snarkOS library.

// The snarkOS library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkOS library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkOS library. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    curves::Field,
    gadgets::r1cs::{ConstraintSystem, Index, LinearCombination, Variable},
};
use snarkos_errors::gadgets::SynthesisError;

use fxhash::FxBuildHasher;
use indexmap::{map::Entry, IndexMap, IndexSet};

use std::{borrow::Borrow, collections::VecDeque, ops::Deref, rc::Rc};

#[derive(Debug, Clone)]
enum NamedObject {
    Constraint(usize),
    Var(Variable),
    // contains the list of named objects that belong to it
    Namespace(Namespace),
}

#[derive(Debug, Clone, Default)]
struct Namespace {
    children: Vec<NamedObject>,
    idx: usize,
}

impl Namespace {
    fn push(&mut self, child: NamedObject) {
        self.children.push(child);
    }
}

type InternedField = usize;
type InternedPathSegment = usize;
type NamespaceIndex = usize;

// a helper object containing a list of values that, when removed, leave a "hole" in their
// place; this allows all the following indices to remain unperturbed; the holes take priority
// when inserting new objects
#[derive(Default)]
pub struct OptionalVec<T> {
    // a list of optional values
    values: Vec<Option<T>>,
    // a double-ended list of indices of the Nones in the values vector
    holes: VecDeque<usize>,
}

impl<T> OptionalVec<T> {
    // inserts a new value either into the first existing hole or extending the vector
    // of values, i.e. pushing it to its end
    #[inline]
    pub fn insert(&mut self, elem: T) -> usize {
        let idx = self.holes.pop_front().unwrap_or_else(|| self.values.len());
        if idx < self.values.len() {
            self.values[idx] = Some(elem);
        } else {
            self.values.push(Some(elem));
        }
        idx
    }

    // returns the index of the next value inserted into the OptionalVec
    #[inline]
    pub fn next_idx(&self) -> usize {
        self.holes.front().copied().unwrap_or_else(|| self.values.len())
    }

    // removes a value at the specified index; assumes that the index points to
    // an existing value that is a Some (i.e. not a hole)
    #[allow(dead_code)]
    pub fn remove(&mut self, idx: usize) -> T {
        let val = self.values[idx].take();
        self.holes.push_back(idx);
        val.unwrap()
    }

    // iterates over all the Some values in the list
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.values.iter().filter(|v| v.is_some()).map(|v| v.as_ref().unwrap())
    }
}

impl<T> std::ops::Index<usize> for OptionalVec<T> {
    type Output = T;

    fn index(&self, idx: usize) -> &Self::Output {
        self.values[idx].as_ref().unwrap()
    }
}

impl<T> std::ops::IndexMut<usize> for OptionalVec<T> {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        self.values[idx].as_mut().unwrap()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InternedPath(Rc<[InternedPathSegment]>);

impl From<Vec<InternedPathSegment>> for InternedPath {
    fn from(v: Vec<InternedPathSegment>) -> Self {
        Self(Rc::from(v))
    }
}

impl Deref for InternedPath {
    type Target = [InternedPathSegment];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<[InternedPathSegment]> for InternedPath {
    fn borrow(&self) -> &[InternedPathSegment] {
        &self.0
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct TestConstraint {
    interned_path: InternedPath,
    a: Vec<(Variable, InternedField)>,
    b: Vec<(Variable, InternedField)>,
    c: Vec<(Variable, InternedField)>,
}

/// Constraint system for testing purposes.
pub struct TestConstraintSystem<F: Field> {
    // used to intern namespace segments
    interned_path_segments: IndexSet<String, FxBuildHasher>,
    // used to intern fields belonging to F
    interned_fields: IndexSet<F, FxBuildHasher>,
    // contains named objects bound to their (interned) paths; the indices are
    // used for NamespaceIndex lookups
    named_objects: IndexMap<InternedPath, NamedObject, FxBuildHasher>,
    // a stack of current path's segments and the index of the current path's
    // index in the named_objects map
    current_namespace: (Vec<InternedPathSegment>, NamespaceIndex),
    // the list of currently applicable constraints
    constraints: OptionalVec<TestConstraint>,
    // the list of currently applicable input variables
    inputs: OptionalVec<InternedField>,
    // the list of currently applicable auxiliary variables
    aux: OptionalVec<InternedField>,
}

impl<F: Field> Default for TestConstraintSystem<F> {
    fn default() -> Self {
        let mut interned_path_segments = IndexSet::with_hasher(FxBuildHasher::default());
        let path_segment = "ONE".to_owned();
        let interned_path_segment = interned_path_segments.insert_full(path_segment).0;
        let interned_path: InternedPath = vec![interned_path_segment].into();

        let mut named_objects = IndexMap::with_hasher(FxBuildHasher::default());
        named_objects
            .insert_full(interned_path, NamedObject::Var(TestConstraintSystem::<F>::one()))
            .0;

        let mut interned_fields = IndexSet::with_hasher(FxBuildHasher::default());
        let interned_field = interned_fields.insert_full(F::one()).0;

        let mut inputs: OptionalVec<InternedField> = Default::default();
        inputs.insert(interned_field);

        let constraints = OptionalVec {
            values: Default::default(),
            holes: Default::default(),
        };

        TestConstraintSystem {
            interned_fields,
            interned_path_segments,
            named_objects,
            current_namespace: Default::default(),
            constraints,
            inputs,
            aux: Default::default(),
        }
    }
}

impl<F: Field> TestConstraintSystem<F> {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn intern_path(&self, path: &str) -> InternedPath {
        let mut vec = vec![];

        for segment in path.split('/') {
            vec.push(self.interned_path_segments.get_index_of(segment).unwrap());
        }

        vec.into()
    }

    fn unintern_path(&self, interned_path: &InternedPath) -> String {
        let mut ret = String::new();
        let mut iter = interned_path.iter().peekable();

        while let Some(interned_segment) = iter.next() {
            ret.push_str(self.interned_path_segments.get_index(*interned_segment).unwrap());
            if iter.peek().is_some() {
                ret.push('/');
            }
        }

        ret
    }

    pub fn print_named_objects(&self) {
        let mut path = String::new();
        for TestConstraint { interned_path, .. } in self.constraints.iter() {
            for interned_segment in interned_path.iter() {
                path.push_str(self.interned_path_segments.get_index(*interned_segment).unwrap());
            }

            println!("{}", path);
            path.clear();
        }
    }

    fn eval_lc(&self, terms: &[(Variable, InternedField)]) -> F {
        let mut acc = F::zero();

        for &(var, interned_coeff) in terms {
            let interned_tmp = match var.get_unchecked() {
                Index::Input(index) => self.inputs[index],
                Index::Aux(index) => self.aux[index],
            };
            let mut tmp = *self.interned_fields.get_index(interned_tmp).unwrap();
            let coeff = self.interned_fields.get_index(interned_coeff).unwrap();

            tmp.mul_assign(coeff);
            acc.add_assign(&tmp);
        }

        acc
    }

    pub fn which_is_unsatisfied(&self) -> Option<String> {
        for TestConstraint { interned_path, a, b, c } in self.constraints.iter() {
            let mut a = self.eval_lc(a.as_ref());
            let b = self.eval_lc(b.as_ref());
            let c = self.eval_lc(c.as_ref());

            a.mul_assign(&b);

            if a != c {
                return Some(self.unintern_path(&interned_path));
            }
        }

        None
    }

    #[inline]
    pub fn is_satisfied(&self) -> bool {
        self.which_is_unsatisfied().is_none()
    }

    #[inline]
    pub fn num_constraints(&self) -> usize {
        self.constraints.iter().count()
    }

    pub fn set(&mut self, path: &str, to: F) {
        let interned_path = self.intern_path(path);
        let interned_field = self.interned_fields.insert_full(to).0;

        match self.named_objects.get(&interned_path) {
            Some(&NamedObject::Var(ref v)) => match v.get_unchecked() {
                Index::Input(index) => self.inputs[index] = interned_field,
                Index::Aux(index) => self.aux[index] = interned_field,
            },
            Some(e) => panic!(
                "tried to set path `{}` to value, but `{:?}` already exists there.",
                path, e
            ),
            _ => panic!("no variable exists at path: {}", path),
        }
    }

    pub fn get(&mut self, path: &str) -> F {
        let interned_path = self.intern_path(path);

        let interned_field = match self.named_objects.get(&interned_path) {
            Some(&NamedObject::Var(ref v)) => match v.get_unchecked() {
                Index::Input(index) => self.inputs[index],
                Index::Aux(index) => self.aux[index],
            },
            Some(e) => panic!(
                "tried to get value of path `{}`, but `{:?}` exists there (not a variable)",
                path, e
            ),
            _ => panic!("no variable exists at path: {}", path),
        };

        *self.interned_fields.get_index(interned_field).unwrap()
    }

    #[inline]
    fn set_named_obj(&mut self, interned_path: InternedPath, to: NamedObject) -> NamespaceIndex {
        match self.named_objects.entry(interned_path) {
            Entry::Vacant(e) => {
                let ns_idx = e.index();
                e.insert(to);
                ns_idx
            }
            Entry::Occupied(e) => {
                let interned_segments = e.remove_entry().0;
                panic!(
                    "tried to create object at existing path: {}",
                    self.unintern_path(&interned_segments)
                );
            }
        }
    }

    #[inline]
    fn compute_path(&mut self, new_segment: &str) -> InternedPath {
        let mut vec = Vec::with_capacity(self.current_namespace.0.len() + 1);
        vec.extend_from_slice(&self.current_namespace.0);
        let (interned_segment, new) = self.interned_path_segments.insert_full(new_segment.to_owned());

        // only perform the check for segments not seen before
        assert!(!new || !new_segment.contains('/'), "'/' is not allowed in names");

        vec.push(interned_segment);

        vec.into()
    }

    #[cfg(not(debug_assertions))]
    fn purge_namespace(&mut self, namespace: Namespace) {
        for child_obj in namespace.children {
            match child_obj {
                NamedObject::Var(var) => match var.get_unchecked() {
                    Index::Aux(idx) => {
                        self.aux.remove(idx);
                    }
                    Index::Input(idx) => {
                        self.inputs.remove(idx);
                    }
                },
                NamedObject::Constraint(idx) => {
                    self.constraints.remove(idx);
                }
                NamedObject::Namespace(children) => {
                    self.purge_namespace(children);
                }
            }
            self.named_objects.swap_remove_index(namespace.idx);
        }
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn purge_namespace(&mut self, _namespace: Namespace) {
        // don't perform a full cleanup in test conditions, so that all the variables and
        // constraints remain available throughout the tests
    }

    #[inline]
    fn register_object_in_namespace(&mut self, named_obj: NamedObject) {
        if let NamedObject::Namespace(ref mut ns) =
            self.named_objects.get_index_mut(self.current_namespace.1).unwrap().1
        {
            ns.push(named_obj);
        }
    }
}

impl<F: Field> ConstraintSystem<F> for TestConstraintSystem<F> {
    type Root = Self;

    fn alloc<Fn, A, AR>(&mut self, annotation: A, f: Fn) -> Result<Variable, SynthesisError>
    where
        Fn: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let interned_path = self.compute_path(annotation().as_ref());
        let interned_field = self.interned_fields.insert_full(f()?).0;
        let index = self.aux.insert(interned_field);
        let var = Variable::new_unchecked(Index::Aux(index));
        let named_obj = NamedObject::Var(var);
        self.register_object_in_namespace(named_obj.clone());
        self.set_named_obj(interned_path, named_obj);

        Ok(var)
    }

    fn alloc_input<Fn, A, AR>(&mut self, annotation: A, f: Fn) -> Result<Variable, SynthesisError>
    where
        Fn: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: AsRef<str>,
    {
        let interned_path = self.compute_path(annotation().as_ref());
        let interned_field = self.interned_fields.insert_full(f()?).0;
        let index = self.inputs.insert(interned_field);
        let var = Variable::new_unchecked(Index::Input(index));
        let named_obj = NamedObject::Var(var);
        self.register_object_in_namespace(named_obj.clone());
        self.set_named_obj(interned_path, named_obj);

        Ok(var)
    }

    fn enforce<A, AR, LA, LB, LC>(&mut self, annotation: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: AsRef<str>,
        LA: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LB: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LC: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
    {
        let interned_path = self.compute_path(annotation().as_ref());
        let index = self.constraints.next_idx();
        let named_obj = NamedObject::Constraint(index);
        self.register_object_in_namespace(named_obj.clone());
        self.set_named_obj(interned_path.clone(), named_obj);

        let mut intern_fields = |uninterned: Vec<(Variable, F)>| -> Vec<(Variable, InternedField)> {
            uninterned
                .into_iter()
                .map(|(var, field)| {
                    let interned_field = self.interned_fields.insert_full(field).0;
                    (var, interned_field)
                })
                .collect()
        };

        let a = intern_fields(a(LinearCombination::zero()).0);
        let b = intern_fields(b(LinearCombination::zero()).0);
        let c = intern_fields(c(LinearCombination::zero()).0);

        self.constraints.insert(TestConstraint { interned_path, a, b, c });
    }

    fn push_namespace<NR: AsRef<str>, N: FnOnce() -> NR>(&mut self, name_fn: N) {
        let name = name_fn();
        let interned_path = self.compute_path(name.as_ref());
        let new_segment = *interned_path.0.last().unwrap();
        let named_obj = NamedObject::Namespace(Default::default());
        self.register_object_in_namespace(named_obj.clone());
        let namespace_idx = self.set_named_obj(interned_path, named_obj);
        if let NamedObject::Namespace(ref mut ns) = self.named_objects[namespace_idx] {
            ns.idx = namespace_idx;
        };

        self.current_namespace.0.push(new_segment);
        self.current_namespace.1 = namespace_idx;
    }

    fn pop_namespace(&mut self) {
        let namespace = if let NamedObject::Namespace(no) = self
            .named_objects
            .swap_remove_index(self.current_namespace.1)
            .unwrap()
            .1
        {
            no
        } else {
            unreachable!()
        };

        // remove object belonging to the popped namespace
        self.purge_namespace(namespace);

        // update the current namespace
        assert!(self.current_namespace.0.pop().is_some());
        if let Some(new_ns_idx) = self.named_objects.get_index_of(self.current_namespace.0.as_slice()) {
            self.current_namespace.1 = new_ns_idx;
        } else {
            // we must be at the "bottom" namespace
            self.current_namespace.1 = 0;
        }
    }

    #[inline]
    fn get_root(&mut self) -> &mut Self::Root {
        self
    }

    #[inline]
    fn num_constraints(&self) -> usize {
        self.num_constraints()
    }
}
