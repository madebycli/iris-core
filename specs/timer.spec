# Timers.

## digits
> Timer für 5 Minuten
< 5 Minuten
= timer "Timer" 300s

## number words
> Stell nen Timer auf zehn Minuten
= timer "Timer" 600s

## named, in one word
> Nudeltimer 8 Minuten
< Nudel
= timer "Nudel" 480s

## named, in two
> Timer für Nudeln 10 Minuten
= timer "Nudeln" 600s

## half an hour
> Stelle einen Timer auf eine halbe Stunde
= timer "Timer" 1800s

## hours and minutes
> Timer 1 Stunde 30 Minuten
= timer "Timer" 5400s

## guided
> Erstelle einen Timer
< Wie lange
= kind question
> 10 Minuten
= timer "Timer" 600s

## guided, a bare number is minutes
> Timer
> fünf
= timer "Timer" 300s

## cancel in the middle
> Erstelle einen Timer
> Stopp
= count timer 0

## how long is left
> Timer für 10 Minuten
@advance 4m
> Wie lange läuft der Timer noch
< 6 Minuten

## end it
> Timer für 5 Minuten
> Beende den Timer
< beendet
= count timer 0

## two timers: which one
> Timer für Nudeln 10 Minuten
> Timer für Eier 6 Minuten
> Beende den Timer
= kind question
> Eier
= count timer 1
= timer "Nudeln"

## two timers: all
> Timer für Nudeln 10 Minuten
> Timer für Eier 6 Minuten
> Beende alle Timer
= count timer 0

## nothing runs
> Wie lange läuft der Timer noch
< kein Timer

## English
@lang en
> Set a timer for 10 minutes
= timer "Timer" 600s

## English, named
@lang en
> Set a pasta timer for 8 minutes
= timer "Pasta" 480s

## a clock time for a timer is a timer until then (from the phone log)
> Erstelle mir ein Timer für 16.30 Uhr morgen
< 16:30
= timer "Timer" 2026-11-11T16:30
= count timer 1

## the clock time is not the name
> Stelle einen Timer für Nudeln bis 17 Uhr
= timer "Nudeln" 2026-11-10T17:00


## undo takes back a timer
> Timer für 5 Minuten
> Rückgängig
= count timer 0

## "sag das nochmal" repeats the last answer
> Timer für 5 Minuten
> Sag das nochmal
< Timer für 5 Minuten

## "wiederhole das" does the last thing again
> Timer für 5 Minuten
> Wiederhole das
= count timer 2

## nothing to repeat
> Sag das nochmal
= kind unhandled
