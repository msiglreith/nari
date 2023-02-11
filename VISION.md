Design vision for this project - I would be happy if I manage to implement at least a fraction of it

# Workspace

The overall vision of the project encloses providing an environment allowing to deliver products in a collaborative and controllable way. A project includes building and maintaining a stack which should be extendable in both directions: higher-level (e.g tools for data authoring) and lower-level (e.g custom domain specific compiler extensions).

- workspace == project
- providing tools for team maintained in sync with product development
- `nari` providing a `canvas` and core structures allowing build customized workspaces tailored towards specific products

# Graph Structure

- extendable graph structure allowing to add metadata (e.g version control, timestamps, author, permissions)
- visualization for editing and authoring
- optimizations as transformation/extension of the graph with equivalent subtrees

# Collaboration

- sync (automatic merge, trust-based) vs async (semi-automatic, review)

# Execution Engine

- multi-stage build process
- incremental changes to graph structure
- code generation should be natural (e.g declarative interface for data)