OK so I want you to add a method that adds a node inside this tag.

The node will show a paragraph.svg icon absolutely positioned at the bottom right.
Add 1/2 --padding away from bottom right corner.
Icon should be visible at 30% so mostly transparent.
It becomes 100% visible on hover and cursor pointer.
When we click on it it becomes mic-mute-fill.svg (always 100% visible even without hover) and a text area shows absolutely positioned, same width as the terminal with --padding space between left terminal side, bottom terminal side, and the icon on the right.

Now there are 4 states
1. button not clicked, paragraph.svg shows, with transparency unless hover
2. paragraph button clicked, now two icons show: send-fill.svg 30% visible and mic-mute-fill.svg, textarea empty
3.1 user types in textarea, send icon becomes 100% visible cusor pointer when textarea is not empty (so it's a signal that depends on the state of the textarea, and the textarea sends updates to a signal which is updated on e.g. keydown events). Clicking on send icon it triggers the content of the textarea to be pasted in the terminal
3.2 user clicks the mic-mute-fill.svg icon, now this icon becomes mic-fill.svg icon and we use the browser API to record speach to text into the textarea. Clicking mic-fill.svg stops the recording. Send button becomes active at any time textarea is not empty to paste the text into the terminal
