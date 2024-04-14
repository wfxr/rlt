## [0.1.1-rc.1](https://github.com/wfxr/rlt/compare/v0.1.1-alpha.6..v0.1.1-rc.1) (2024-04-14)

### 🚀 Features

- Implement logical clock ([#6](https://github.com/wfxr/rlt/issues/6)) - ([d90afd8](https://github.com/wfxr/rlt/commit/d90afd833490de50de9aae82b6cb01cf456a3290))
- Add debug derive to BenchCli ([#7](https://github.com/wfxr/rlt/issues/7)) - ([26208b8](https://github.com/wfxr/rlt/commit/26208b8939907ebd079b1b6e267979c76e610146))

### 🐛 Bug Fixes

- Fix panic when the receiver is dropped([#2](https://github.com/wfxr/rlt/issues/2)) - ([08f0bcd](https://github.com/wfxr/rlt/commit/08f0bcd94a19e819522a19ad69b50e04ba830ab7))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-rc.1 - ([d67a49c](https://github.com/wfxr/rlt/commit/d67a49c9989acd7cbe392f01e9230fd1f0241bce))
- Use git-cliff to generate changelog ([#8](https://github.com/wfxr/rlt/issues/8)) - ([13d806c](https://github.com/wfxr/rlt/commit/13d806cb4a3c63db09e93735d2fbcf785ed9d22e))
- Remove spaces in tui title - ([722842e](https://github.com/wfxr/rlt/commit/722842ec9e25e11de45c6de24482ed5cf94ee4c0))

## [0.1.1-alpha.6](https://github.com/wfxr/rlt/compare/v0.1.1-alpha.5..v0.1.1-alpha.6) (2024-04-12)

### 🚀 Features

- Add log framework - ([2cc1e5d](https://github.com/wfxr/rlt/commit/2cc1e5d525875df48969837e1f75d490bf4238f0))

### 🚜 Refactor

- Remove unnecessary log messages - ([0e754a6](https://github.com/wfxr/rlt/commit/0e754a6488d33a639aa7fafd48754bd0c0d145f8))
- Simplify key event handling for log win - ([3293398](https://github.com/wfxr/rlt/commit/329339877908c579bb41932f5f1d8d855bffbd1f))
- Make log feature optional - ([61f6182](https://github.com/wfxr/rlt/commit/61f6182039274ef9893ca5eaf639a77b404c8d54))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-alpha.6 - ([7691f1b](https://github.com/wfxr/rlt/commit/7691f1b2d3d265c58d81477d5d07869fb6dd9021))

## [0.1.1-alpha.5](https://github.com/wfxr/rlt/compare/v0.1.1-alpha.4..v0.1.1-alpha.5) (2024-04-06)

### 📚 Documentation

- Update description - ([ecd59d6](https://github.com/wfxr/rlt/commit/ecd59d606cd97dc10091a883d707b7b2964ff766))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-alpha.5 - ([f80cfb0](https://github.com/wfxr/rlt/commit/f80cfb0e372d7ce5fa2c86910a5dcea0e79c1ab3))

## [0.1.1-alpha.4](https://github.com/wfxr/rlt/compare/v0.1.1-alpha.3..v0.1.1-alpha.4) (2024-04-06)

### 📚 Documentation

- Update description - ([6689648](https://github.com/wfxr/rlt/commit/66896482cd11998c32b39aa66b7bdf1b81f3573d))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-alpha.4 - ([6259163](https://github.com/wfxr/rlt/commit/62591631dd09d4a042262a0cc89af416df7b818d))

## [0.1.1-alpha.3](https://github.com/wfxr/rlt/compare/v0.1.1-alpha.2..v0.1.1-alpha.3) (2024-04-05)

### 🚀 Features

- Add postgres example - ([3830f7b](https://github.com/wfxr/rlt/commit/3830f7b6db2c980041fac9c6516f05fd8df1f99f))

### 🚜 Refactor

- Simplify the cli interface - ([d4f2d09](https://github.com/wfxr/rlt/commit/d4f2d09caa45dd15dca8eab7898db5b15131d660))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-alpha.3 - ([c339f99](https://github.com/wfxr/rlt/commit/c339f99739ed9ae461ab3ae94d21fdd8bea59f72))

## [0.1.1-alpha.2](https://github.com/wfxr/rlt/compare/v0.1.1-alpha.1..v0.1.1-alpha.2) (2024-04-05)

### 🚀 Features

- Add http feature - ([4093223](https://github.com/wfxr/rlt/commit/40932231c90ea406d75d0d78c5821c967b2ccfe7))
- Add http_reqwest example - ([e87d845](https://github.com/wfxr/rlt/commit/e87d8452b79e61bc6e7ad1bad21b33733ff67660))
- Change the signature of `teardown` method - ([07a40e8](https://github.com/wfxr/rlt/commit/07a40e82da28e9ff1d2c9192b09d9afa1797091f))
- Remove rand dependency - ([df63280](https://github.com/wfxr/rlt/commit/df632806437481cafe4ed421a159efbbe60004db))
- Add setup and teardown hooks to BenchSuite - ([1318d10](https://github.com/wfxr/rlt/commit/1318d10d47dc22cd336ced57f418ff0527489b5f))
- Add worker_id to BenchSuite::state() - ([0efab5c](https://github.com/wfxr/rlt/commit/0efab5c467763e3b449d57b2eddbcaab2e47994c))
- Change the separator of the histogram - ([e616ab1](https://github.com/wfxr/rlt/commit/e616ab1f0092bfd5607661c0ee9baaf6340fced4))
- Improve the tui and text reporter output - ([deb61a8](https://github.com/wfxr/rlt/commit/deb61a8e61ee546bd67f5c3313663c99111cafeb))
- Improve time unit display in tui - ([590b349](https://github.com/wfxr/rlt/commit/590b349f17617a3021aa60fd3ef35ac25ce410cb))
- Make runner state mutable - ([13df38d](https://github.com/wfxr/rlt/commit/13df38db5dfc890a7e5a6e0479884cd94d675ccf))
- Add StatelessBenchSuite trait - ([56d3386](https://github.com/wfxr/rlt/commit/56d3386fd728f24296c74f4943f1311affeca3e6))
- Add simple example - ([2a3e8d5](https://github.com/wfxr/rlt/commit/2a3e8d54256c4fc21c168ca234d3e49509a53a97))

### 🐛 Bug Fixes

- Make sure all tasks are joined - ([4e0503c](https://github.com/wfxr/rlt/commit/4e0503c1a51dd0c7a2d7cbb1c0108e559143e3c0))
- Fix hang in case iterations reached first - ([5028a70](https://github.com/wfxr/rlt/commit/5028a705a2a79b9f6cd37b1a035927488204a278))
- Fix indent for text reporter - ([a179715](https://github.com/wfxr/rlt/commit/a17971551a687509442b72afb29d46d99a349250))
- Fix auto time window selection - ([0adfc59](https://github.com/wfxr/rlt/commit/0adfc5942a2180e2af387a42c5a459b813aed00d))

### 🚜 Refactor

- Rename - ([1578a06](https://github.com/wfxr/rlt/commit/1578a067bea0578dd79f2d119af8f1899d7b7bba))
- Avoid extra newline in text reporter - ([91d06b2](https://github.com/wfxr/rlt/commit/91d06b242228db612e10297557fe507b1283b3cd))
- Remove init method from BenchSuite - ([bff5e10](https://github.com/wfxr/rlt/commit/bff5e10a4f5e9b0e46cbc843112cdb0bbc5907b4))
- Rename RunnerState to WorkerInfo - ([e686770](https://github.com/wfxr/rlt/commit/e686770343f7ebae33c4b0e643e8bc399c03f9c4))
- Use JoinSet instead of Vec<JoinHandle> - ([4cf851e](https://github.com/wfxr/rlt/commit/4cf851e38d02f24a6b58a3f2312764c112015132))
- Update summary format for text reporter - ([eb0483c](https://github.com/wfxr/rlt/commit/eb0483caec147c04e6a87448dd8f68ca747f6575))
- Rename example - ([eead358](https://github.com/wfxr/rlt/commit/eead358d75a3734379d5fcd643f2cc685a312265))
- Remove unnecessary init function - ([b7380e6](https://github.com/wfxr/rlt/commit/b7380e6fa321faa0a53f9d86f6b674fa900a8643))
- Rename UnknownError status - ([c4b7739](https://github.com/wfxr/rlt/commit/c4b773929bfd77ec9cc3adb71dc16ec65faa4845))

### 📚 Documentation

- Improve README.md - ([b5f0bb3](https://github.com/wfxr/rlt/commit/b5f0bb3ae702db97dfb67852d2a005fcb895a058))
- Update README.md - ([97fffe6](https://github.com/wfxr/rlt/commit/97fffe60e5d26a37a076d87f64a34d1a4383d108))
- Add `no_run` mark - ([1baf83a](https://github.com/wfxr/rlt/commit/1baf83a75a362b45d957b478895c1f24d62a6435))
- Add more docs - ([6727033](https://github.com/wfxr/rlt/commit/672703349b5e5dea8aafa21c2b85a0d506fe7ec6))
- Add docs for the CLI module - ([dbea9e7](https://github.com/wfxr/rlt/commit/dbea9e7b17f2ac186f49be0871e6dae3927412a4))
- Update README.md - ([06ab23f](https://github.com/wfxr/rlt/commit/06ab23f7ecde17e8f22c7a184be17450154a8371))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-alpha.2 - ([9bb7eb4](https://github.com/wfxr/rlt/commit/9bb7eb4204d57e4e8d2f7260bf71e9f57176f275))

## [0.1.1-alpha.1] - 2024-03-26

### 🐛 Bug Fixes

- Fix hang when canceling a paused bench - ([be28f12](https://github.com/wfxr/rlt/commit/be28f1229ce03df50a423ad705c4cac4ebd4d846))

### 🚜 Refactor

- Update module visibility - ([71d04a6](https://github.com/wfxr/rlt/commit/71d04a644c9dacf340aa8e63ae91a497c9fc3515))

### 📚 Documentation

- Update readme - ([fe1b655](https://github.com/wfxr/rlt/commit/fe1b655b8ee283d7d26d881f6af937a29c0e0215))

### ⚙️ Miscellaneous Tasks

- Release rlt version 0.1.1-alpha.1 - ([9a022bd](https://github.com/wfxr/rlt/commit/9a022bd0ffff84d686681c72c3fd3bf6c15f817a))
- Disable minimal version check - ([59993f3](https://github.com/wfxr/rlt/commit/59993f33efd6767e6f054acc6295e75dc9a36ac1))
- Remove unused dependency - ([848407b](https://github.com/wfxr/rlt/commit/848407bb5ef1623d3f22d4062b0d5b9645466a28))
- Simplify workflows - ([5b323b2](https://github.com/wfxr/rlt/commit/5b323b24e24d3fd01d17604b017ff6cc68504df8))
- Update rustfmt & rust-toolchain - ([b56672f](https://github.com/wfxr/rlt/commit/b56672f5bf9770eca11300d624303ad646014dee))
- Setup - ([43bccb6](https://github.com/wfxr/rlt/commit/43bccb61b7d996827a6dc07b92453080594e66da))
- Add rustfmt config file - ([f345461](https://github.com/wfxr/rlt/commit/f3454616a42a870d71853abb15aa7ddd52bede72))

<!-- generated by git-cliff -->
