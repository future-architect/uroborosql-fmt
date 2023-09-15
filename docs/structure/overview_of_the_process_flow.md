# Overview of the process flow

<img src="../../images/process_flow.png"/>

1. Parse input SQL with tree-sitter-sql [tree-sitter-sql](https://github.com/future-architect/tree-sitter-sql) to get CST.
2. Parsing CSTs and converting them into their original tree structure.
3. Formatting and output using original tree structure.
