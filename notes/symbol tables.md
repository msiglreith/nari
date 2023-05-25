- mapping `Type -> u32` allowing fast compare operations O(1) and reduce storage
- `union` together same data (equal + hash)
- hashset (hash of `Type` + idx)
- perf: hash the type value

### References
- [https://github.com/mwillsey/symbol_table](https://github.com/mwillsey/symbol_table)
- [https://matklad.github.io/2020/03/22/fast-simple-rust-interner.html](https://matklad.github.io/2020/03/22/fast-simple-rust-interner.html)
- [https://github.com/CAD97/simple-interner](https://github.com/CAD97/simple-interner)


