use crate::orchestrator::Orchestrator;
use crate::TaskId;

impl Orchestrator {
    pub(crate) fn has_cycle(&self) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();

        fn dfs(
            node: TaskId,
            tasks: &std::collections::HashMap<TaskId, crate::task::MetaTask>,
            visited: &mut std::collections::HashSet<TaskId>,
            rec_stack: &mut std::collections::HashSet<TaskId>,
        ) -> bool {
            if rec_stack.contains(&node) {
                return true;
            }
            if visited.contains(&node) {
                return false;
            }

            visited.insert(node);
            rec_stack.insert(node);

            if let Some(task) = tasks.get(&node) {
                for &parent in &task.depends_on {
                    if dfs(parent, tasks, visited, rec_stack) {
                        return true;
                    }
                }
            }

            rec_stack.remove(&node);
            false
        }

        for &id in self.tasks.keys() {
            if dfs(id, &self.tasks, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        false
    }
}
