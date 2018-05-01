//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

//! Error module, containing error types
error_chain!{
    errors {
        /// Error kind indicating that the JSON parser failed
        ParserError {
            description("Failed to create a Task from JSON")
        }
        /// Error kind indicating that the Reader failed to read something
        ReaderError {
            description("Failed to read tasks from a Reader")
        }
        /// Error kind indicating that a call to the task warrior binary failed
        TaskCmdError {
            description("There was a problem while calling the external 'task' binary")
        }
        /// Error kind indicating that a conversion to JSON failed
        SerializeError {
            description("A Task could not be converted to JSON")
        }
        /// Error kind indicating that there was an internal Error in the TaskCache where a Task
        /// was not cache, which was expected to be there. This is a Bug, which should be reported.
        CacheMissError {
            description("A not cached Task was marked as dirty. This is a BUG! Report to task-hookrs maintainers.")
        }
        /// Error kind indicating that the user tried to do something with a TaskCache which would
        /// lead to loosing unsaved changes
        DirtyCacheError {
            description("Tried to discard unsaved changes in TaskCache.")
        }
    }
}
