# Alarms. Clock of every case: Tuesday 2026-11-10, 16:00, Europe/Berlin.

## a clock time
> Stell einen Wecker auf 7 Uhr
< 7:00
= alarm 2026-11-11T07:00

## in words
> Weck mich um halb sieben
= alarm 2026-11-11T06:30

## a bare hour is taken as said, not guessed (unlike a reminder)
> Stelle einen Wecker auf fünf
= alarm 2026-11-11T05:00

## later today
> Weck mich um 18 Uhr
= alarm 2026-11-10T18:00

## relative (from the phone log: "Und erstelle einen Wecker in 10 Minuten")
> Erstelle einen Wecker in 10 Minuten
= alarm 2026-11-10T16:10

## tomorrow
> Wecker für morgen um 6:45
= alarm 2026-11-11T06:45

## guided: the time is asked
> Stell einen Wecker
< Wann
= kind question
> um 8 Uhr
= alarm 2026-11-11T08:00

## cancel while asked
> Stell einen Wecker
> Abbrechen
= alarms 0

## the clock app gives no receipt: the reply says so
@alarms unconfirmed
> Stell einen Wecker auf 7 Uhr
< noch nicht bestätigt
= alarm 2026-11-11T07:00

## no clock app: nothing is claimed
@clock missing
> Stell einen Wecker auf 7 Uhr
< keine Uhr-App
= kind failed
= alarms 0

## next alarm
> Stell einen Wecker auf 7 Uhr
> Wann klingelt mein nächster Wecker
< morgen um 7:00 Uhr

## no alarm
> Wann klingelt mein nächster Wecker
< keinen Wecker

## the list opens in the clock app
> Zeig mir meine Wecker
< Weckerliste

## English
@lang en
> Wake me at 6:30
= alarm 2026-11-11T06:30

## "Wecker" inside a reminder is its title (from the phone log)
> Erinnere mich in 10 Minuten daran, den Wecker auszumachen
= alarms 0
= reminder "Wecker ausmachen" 2026-11-10T16:10

## a note about an alarm is a note
> Notiere dass ich einen Wecker stellen muss
= alarms 0
= note "Ich einen Wecker stellen muss"
