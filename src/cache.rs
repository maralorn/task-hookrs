//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! This module offers a cache for tasks. This makes it possible to run complex bulk operations on tasks,
//! while minimizing external process calls.
use uuid::Uuid;
use task::Task;
use status::TaskStatus;
use std::collections::{HashMap, HashSet};
use error::{ErrorKind as Ek, Result, ResultExt};
use tw::{query, save};

/// A TaskCache caches tasks.
/// A TaskCache has a list of states, which it ignores.
/// That means tasks in that state won't be requested from taskwarrior.
/// This will give a performance advantage, when ignoring completed and deleted tasks.
/// Note, that when the program makes changes on the cache there still might be tasks in ignored
/// states in the cache. They will be saved on calling `write()` regardless of there new state.
#[derive(Clone, Debug)]
pub struct TaskCache {
    dirty: HashSet<Uuid>,
    cache: HashMap<Uuid, Task>,
    ignore: Vec<TaskStatus>,
}

fn generate_query(ignore: &Vec<TaskStatus>) -> String {
    ignore
        .iter()
        .map(|x| format!("-{}", x).to_uppercase())
        .collect::<Vec<_>>()
        .join(" ")
}

impl TaskCache {
    /// Creates a new TaskCache
    pub fn new(ignore: Vec<TaskStatus>) -> TaskCache {
        TaskCache {
            dirty: HashSet::default(),
            cache: HashMap::default(),
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
    pub fn load(&mut self) -> Result<&mut TaskCache> {
        if self.dirty.len() > 0 {
            bail!(Ek::DirtyCacheError);
        } else {
            self.cache.clear();
        }
        self.cache.extend(
            query(&generate_query(&self.ignore))?
                .into_iter()
                .map(|t| (t.uuid().clone(), t)),
        );
        Ok(self)
    }

    /// Clears the cache and throws away unsaved changes.
    pub fn reset(&mut self) {
        self.dirty.clear();
        self.cache.clear();
    }

    /// Refreshs the cache, by first saving and then reloading.
    /// This is not only necessary to get out of band changes in taskwarrior
    /// but also because changes to one task may have implications to state of another one.
    pub fn refresh(&mut self) -> Result<&mut TaskCache> {
        self.write()?;
        self.load()
    }

    /// Saves all entries marked as dirty.
    pub fn write(&mut self) -> Result<&mut TaskCache> {
        if self.dirty.len() > 0 {
            save(self.dirty
                .iter()
                .map(|uuid| self.cache.get(uuid).chain_err(|| Ek::CacheMissError))
                .collect::<Result<Vec<_>>>()?)?;
            self.dirty.clear();
        }
        Ok(self)
    }

    /// Gives all tasks matching the given filter.
    pub fn filter<F>(&self, func: F) -> Vec<&Task>
    where
        F: Fn(&Task) -> bool,
    {
        self.cache.values().filter(|x| func(*x)).collect()
    }

    /// Gives all tasks matching the given filter as mutable references.
    /// Beware! This will mark all matches as dirty. They will be saved on the next `write()`.
    pub fn filter_mut<F>(&mut self, func: F) -> Vec<&mut Task>
    where
        F: Fn(&Task) -> bool,
    {
        let tasks = self.cache
            .values_mut()
            .filter(|x| func(*x))
            .collect::<Vec<_>>();
        self.dirty.extend(tasks.iter().map(|t| t.uuid().clone()));
        tasks
    }

    /// Gives the task with the corresponding uuid.
    pub fn get(&self, uuid: &Uuid) -> Option<&Task> {
        self.cache.get(uuid)
    }

    /// Gives a mutable referecne to the task with the corresponding uuid.
    /// Beware! This will mark that task as dirty. It will be saved on the next `write()`.
    pub fn get_mut(&mut self, uuid: &Uuid) -> Option<&mut Task> {
        let task = self.cache.get_mut(uuid);
        if task.is_some() {
            self.dirty.insert(uuid.clone());
        }
        task
    }

    /// Sets a new task into the cache. It will be marked as dirty and saved on the next `write()`.
    pub fn set(&mut self, task: Task) -> &mut TaskCache {
        self.dirty.insert(task.uuid().clone());
        self.cache.insert(task.uuid().clone(), task);
        self
    }
}

impl Default for TaskCache {
    fn default() -> Self {
        Self::new(vec![TaskStatus::Completed, TaskStatus::Deleted])
    }
}
