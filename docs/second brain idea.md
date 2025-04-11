
App structure:
learner thread: 
 - intercepts keystrokes, clicks (Maybe click targets), takes system snapshots (eg active app), then stores them in a timescale db, 
Thinker thread: 
 - every so often takes that knowledge and creates an entry in a more general DB that takes those key presses, passes them to an LLM and creates a description of user events in that time frame (browsed email, went to wikipedia.com, asked google about leopards)
 Recall thread :
	user can fuzzy find content, or ask about a time frame (eg what was I doing last week, or What was my wikipedia rabbit hole about on thursday) that gets passed either to a fuzzyfind tool or an LLM to create a db query for the correct data stored in timescale
	