# Detailed roadmap

| Task                                                                                                                              | Size | Comment                                                |
|-----------------------------------------------------------------------------------------------------------------------------------|------|--------------------------------------------------------|
| Support debugger out of the box in [Cairo VSCode extension](https://marketplace.visualstudio.com/items?itemName=starkware.cairo1) | 1    | Mostly done                                            |
| Integrate with [snforge](https://foundry-rs.github.io/starknet-foundry/) (only test, no contract calls)                           | 2    | Only a code wise integration, without request handling |
| Add handling of basic DAP requests                                                                                                | 2.5  |                                                        |
| Retrieving values of function args                                                                                                | 2.5  | In practice it is a subtask of the previous task       |
| Support retrieving values of local variables                                                                                      | 5    |                                                        |
| UX stuff: debug lens in CairoLS, better extension UX                                                                              | 1.5  |                                                        |
| Enhance [`scarb execute`](https://docs.swmansion.com/scarb/docs/extensions/execute.html) with debug capabilities                  | 1    | Should be straightforward once above tasks are done    |
| Support debugging contract calls in [snforge](https://foundry-rs.github.io/starknet-foundry/)                                     | 3    |                                                        |

Future plans: support more advanced DAP requests.
