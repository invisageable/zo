# misc — bootlges — github-pages.md.

- zo grammar: @crates/compiler/zo-notes/public/grammar/zo.ebnf

>
> ```zo
> -- create a github page per repository (1 pages site max).
> 
> fun main() {
>
>   imu on_github : </> ::= <>
>     <goto>Settings</goto>
>     <then>Pages</then>
>     <set>{Source = "main"}</set>
>     <set>{Folder = "/" | "/docs"}</set>
>     <click>Save</Save>
>   </>;
>
>   # dom on_github; -- `URL @https://<username>.github.io/<repo-name>`
> }
> ```
>
> results: [@https://invisageable.github.io/zo](https://invisageable.github.io/zo)