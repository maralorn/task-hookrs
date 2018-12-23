//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! This module offers a cache for tasks. This makes it possible to run complex bulk operations on tasks,
//! while minimizing external process calls.
use error::ErrorKind as Ek;
use failure::Fallible as Result;
use status::TaskStatus;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    iter::once,
};

use task::Task;
use tw::{query, save};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum MutationState {
    Dirty,
    Clean,
}

/// A TaskCache caches tasks.
/// For performance reasons a TaskCache can blacklist
/// That means tasks in that state won't be requested from taskwarrior.
/// This will give a performance advantage, when ignoring completed and deleted tasks.
/// Note, that when the program makes changes on the cache there still might be tasks in ignored
/// states in the cache. They will be saved on calling `write()` regardless of their new state.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskCache {
    cache: HashMap<Uuid, RefCell<(Task, MutationState)>>,
    ignore: Vec<TaskStatus>,
}

/// A TaskCell contains a pointer to a Task in the cache. Which can be borrow immutable or mutable.MutationState
/// The calls will return None if a conflicting Borrow is active.
pub struct TaskCell<'a> {
    cell: &'a RefCell<(Task, MutationState)>,
    cache: &'a TaskCache,
}

impl<'a> TaskCell<'a> {
    /// Trys to borrow the Task immutable.
    pub fn borrow(&self) -> Option<Ref<Task>> {
        self.cell
            .try_borrow()
            .ok()
            .map(|x| Ref::map(x, |(task, _)| task))
    }
    /// Trys to borrow the Task mutable.
    pub fn borrow_mut(&self) -> Option<RefMut<Task>> {
        self.cell.try_borrow_mut().ok().map(|x| {
            RefMut::map(x, |(task, state)| {
                *state = MutationState::Dirty;
                task
            })
        })
    }

    /// Gives a reference to the TaskCache this TaskCell belongs to.
    pub fn cache(&self) -> &'a TaskCache {
        self.cache
    }
}

fn generate_query(ignore: &[TaskStatus]) -> String {
    ignore
        .iter()
        .map(|x| format!("-{}", x).to_uppercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn task_to_entry(task: Task) -> (Uuid, RefCell<(Task, MutationState)>) {
    (*task.uuid(), RefCell::new((task, MutationState::Clean)))
}

impl TaskCache {
    /// Creates a new TaskCache
    pub fn new(ignore: Vec<TaskStatus>) -> TaskCache {
        TaskCache {
            cache: HashMap::new(),
            ignore: ignore,
        }
    }

    /// Gives tasks ignored by this TaskCache
    pub fn ignore(&self) -> &Vec<TaskStatus> {
        &self.ignore
    }

    /// Will load all unignored tasks in the cache.
    /// This will throw an error of kind DirtyCacheError, if there are unsaved changes.
    /// Call `reset` first to circumvent this if you need it.
    pub fn load(&mut self) -> Result<()> {
        if self
            .cache
            .iter()
            .any(|(_, x)| (*x.borrow()).1 == MutationState::Dirty)
        {
            bail!(Ek::DirtyCacheError);
        } else {
            self.cache.clear();
        }
        query(&generate_query(&self.ignore))
            .map(|x| x.into_iter().map(task_to_entry))
            .map(|x| self.cache.extend(x))
    }

    /// Clears the cache and throws away unsaved changes.
    pub fn reset(&mut self) {
        self.cache.clear();
    }

    /// Refreshs the cache, by first saving and then reloading.
    /// This is not only necessary to get out of band changes in taskwarrior
    /// but also because changes to one task may have implications to state of another one.
    pub fn refresh(&mut self) -> Result<()> {
        self.write().and_then(|_| self.load())
    }

    /// Saves all entries marked as dirty.
    pub fn write(&mut self) -> Result<()> {
        let updates = self
            .cache
            .values()
            .map(RefCell::borrow)
            .filter(|x| (*x).1 == MutationState::Dirty)
            .collect::<Vec<_>>();
        if updates.is_empty() {
            Ok(())
        } else {
            save(updates.iter().map(|x| &(*x).0))
        }
    }

    /// Gives an Iterator over all tasks in the cache
    pub fn iter(&self) -> impl Iterator<Item = TaskCell> {
        self.cache.values().map(move |x| TaskCell { cell: &x, cache: self })
    }

    /// Gives the task with the corresponding uuid.
    pub fn get_ptr(&self, uuid: &Uuid) -> Option<TaskCell> {
        self.cache.get(uuid).map(|x| TaskCell { cell: &x, cache: self })
    }

    /// Sets a new task into the cache. It will be marked as dirty and saved on the next `write()`.
    pub fn set(&mut self, task: Task) {
        self.cache.extend(once(task_to_entry(task)));
    }
}

impl Default for TaskCache {
    fn default() -> Self {
        Self::new(vec![TaskStatus::Completed, TaskStatus::Deleted])
    }
}
