# 4. Bare hours

**Decision.** For a reminder, an hour said without the half of the day ("um fünf", "at 8"):

- no day named: the next time a clock shows that hour (16:00, "um fünf" → 17:00 today);
- a day named and the hour is 1–6: the afternoon ("morgen um 3" → 15:00);
- a day named, other hours: as said ("morgen um 8" → 8:00);
- a repetition: as said ("täglich um 7" → 7:00);
- said without doubt ("17 Uhr", "08:00", "5 pm", "8 Uhr abends"): as said.

The reply always names the resolved time in full. Alarms use `HalfDay::AsSaid`.

**Why.** The Kotlin version put "um fünf" said in the afternoon at 5:00 the next morning.
Asking "morning or evening?" every time is slower than being right most of the time and
visibly wrong the rest.
