# Reminders in German. The clock of every case is Tuesday 2026-11-10, 16:00, Europe/Berlin.

## in one sentence: day and time
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
< Zahnarzt
= reminder "Zahnarzt" 2026-11-11T08:00

## the title is the short thing, not the sentence
> Erinnere mich um 16:30 Uhr, dass ich den Hund rausbringen muss
= reminder "Hund rausbringen" 2026-11-10T16:30

## verb at the end
> Kannst du mich um 17 Uhr an Oma erinnern
= reminder "Oma" 2026-11-10T17:00

## created as a reminder with the time in the middle
> Erstelle eine Erinnerung für 16:30 Uhr Hund rausbringen
= reminder "Hund rausbringen" 2026-11-10T16:30

## "tell me"
> Ich muss um 17 Uhr zum Zahnarzt, sag mir Bescheid
= reminder "Zahnarzt" 2026-11-10T17:00

## relative
> Erinner mich in 20 Minuten an die Wäsche
= reminder "Wäsche" 2026-11-10T16:20

## half an hour
> Erinnere mich in einer halben Stunde an die Pizza
= reminder "Pizza" 2026-11-10T16:30

## a task is a reminder
> Erstelle eine Aufgabe um 16:30 Uhr Hund rausbringen
= reminder "Hund rausbringen" 2026-11-10T16:30

## a bare hour means the next time the clock shows it
> Erinnere mich um fünf an den Müll
< 17:00
= reminder "Müll" 2026-11-10T17:00

## a bare hour that has passed twice today is tomorrow morning
@now 2026-11-10T22:00 Europe/Berlin
> Erinnere mich um fünf an den Müll
= reminder "Müll" 2026-11-11T05:00

## the part of the day decides
> Erinnere mich um 8 Uhr abends an die Tabletten
= reminder "Tabletten" 2026-11-10T20:00

## halb fünf is 4:30, and the next one is in the afternoon
> Erinnere mich um halb fünf an den Hund
= reminder "Hund" 2026-11-10T16:30

## Viertel nach
> Erinnere mich um Viertel nach sechs an das Essen
= reminder "Essen" 2026-11-10T18:15

## a weekday
> Erinnere mich am Freitag um 15 Uhr an die Abgabe
= reminder "Abgabe" 2026-11-13T15:00

## a date
> Erinnere mich am 3. Dezember um 10 Uhr an den Geburtstag
= reminder "Geburtstag" 2026-12-03T10:00

## a day without a time gets the morning
> Erinnere mich morgen an die Blumen
= reminder "Blumen" 2026-11-11T09:00

## number words in compounds
> Erinnere mich in fünfundvierzig Minuten an den Kuchen
= reminder "Kuchen" 2026-11-10T16:45

## every day
> Erinnere mich täglich um 7 Uhr an die Tabletten
= reminder "Tabletten" 2026-11-11T07:00 repeat=daily

## every Monday
> Erinnere mich jeden Montag um 7 Uhr an den Müll
= reminder "Müll" 2026-11-16T07:00 repeat=days:1

## guided: time first, then the name
> Erstelle eine Erinnerung
< Uhr
= kind question
> um 16:30 Uhr
< heißen
> Hund rausbringen
= reminder "Hund rausbringen" 2026-11-10T16:30
= count reminder 1

## guided: only "remind me"
> Erinnere mich
= kind question
> morgen um 9
> Zahnarzt anrufen
= reminder "Zahnarzt anrufen" 2026-11-11T09:00

## the title is known, the time is asked
> Erinnere mich an die Wäsche
< Wäsche
= kind question
> in 30 Minuten
= reminder "Wäsche" 2026-11-10T16:30

## the time is known, the title is asked
> Erinnere mich um 18 Uhr
< heißen
> Döner essen gehen
= reminder "Döner essen gehen" 2026-11-10T18:00

## cancel ends a guided reminder, in any words
> Erstelle eine Erinnerung
> Abbrechen
< abgebrochen
= count reminder 0

## cancel after the time
> Erstelle eine Erinnerung
> um 16:30 Uhr
> Vergiss es
= count reminder 0

## a time that does not exist is asked again
> Erinnere mich heute um 8 Uhr morgens an den Bus
< vorbei
= kind question
> morgen um 8
= reminder "Bus" 2026-11-11T08:00

## list
> Erinnere mich um 17 Uhr an Oma
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
> Welche Erinnerungen habe ich
< 2 Erinnerungen
< Oma
< Zahnarzt

## list for one day
> Erinnere mich um 17 Uhr an Oma
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
> Welche Erinnerungen habe ich morgen
< Zahnarzt
<! Oma

## next
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
> Erinnere mich um 17 Uhr an Oma
> Was ist meine nächste Erinnerung
< Oma
<! Zahnarzt

## nothing to list
> Welche Erinnerungen habe ich
< keine

## delete by name
> Erinnere mich um 17 Uhr an Oma
> Lösche die Erinnerung Oma
< gelöscht
= count reminder 0

## delete all asks first
> Erinnere mich um 17 Uhr an Oma
> Erinnere mich um 18 Uhr an Opa
> Lösche alle Erinnerungen
= kind question
= count reminder 2
> Ja
= count reminder 0

## delete all, then no
> Erinnere mich um 17 Uhr an Oma
> Lösche alle Erinnerungen
> Nein
= count reminder 1

## delete without a name asks which
> Erinnere mich um 17 Uhr an Oma
> Erinnere mich um 18 Uhr an Opa
> Lösche eine Erinnerung
= kind question
> Opa
= count reminder 1
= reminder "Oma"

## into the task app
@setting reminder.destination tasks
@tasklist Privat
@tasklist Arbeit
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
< Privat
= task "Zahnarzt" 2026-11-11T08:00 list=Privat
= count reminder 0

## into a named list of the task app
@setting reminder.destination tasks
@tasklist Privat
@tasklist Arbeit
> Erinnere mich morgen um 9 Uhr an den Bericht auf meiner Arbeitsliste
= task "Bericht" 2026-11-11T09:00 list=Arbeit

## into the calendar
@setting reminder.destination calendar
> Erinnere mich am Freitag um 15 Uhr an die Abgabe
< Kalender
= event "Abgabe" 2026-11-13T15:00
= count reminder 0

## into both
@setting reminder.destination both
@tasklist Privat
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
= task "Zahnarzt" 2026-11-11T08:00
= event "Zahnarzt" 2026-11-11T08:00

## the task app is down: the reminder is kept in Iris and the reply says so
@setting reminder.destination tasks
@tasks down
> Erinnere mich morgen um 8 Uhr an den Zahnarzt
< nicht erreichbar
= reminder "Zahnarzt" 2026-11-11T08:00

## what is not a reminder is left alone
> Wie wird das Wetter morgen
= kind unhandled
= count reminder 0

## do not forget
> Vergiss nicht den Hund um halb fünf rauszubringen
= reminder "Hund rausbringen" 2026-11-10T16:30

## tell me, time in the middle
> Sag mir um 16:30 Bescheid dass ich den Hund rausbringen muss
= reminder "Hund rausbringen" 2026-11-10T16:30

## a long spoken sentence with a correction
> Ja, stelle mir eine Erinnerung oder erinnere mich an morgen 19.30 Uhr Döner essen gehen
= reminder "Döner essen gehen" 2026-11-11T19:30

## separable verb at the end
> Stelle mir eine Erinnerung für 18 Uhr Wäsche ein
= reminder "Wäsche" 2026-11-10T18:00

## hour and minutes in words
> Erinnere mich übermorgen um 7 Uhr 30 an den Zug
= reminder "Zug" 2026-11-12T07:30

## zu-infinitive
> Erinner mich dran, morgen früh um sieben die Mülltonne rauszustellen
= reminder "Mülltonne rausstellen" 2026-11-11T07:00

## next weekday
> Erinnere mich nächsten Dienstag um 10 an das Meeting
= reminder "Meeting" 2026-11-17T10:00

## this evening without a clock time
> Erinnere mich heute Abend an den Film
= reminder "Film" 2026-11-10T19:00

## minutes to the hour
> Erinnere mich um 5 vor 12 an den Topf
= reminder "Topf" 2026-11-10T23:55

## weekdays
> Erinnere mich werktags um 6:30 Uhr ans Aufstehen
= reminder "Aufstehen" 2026-11-11T06:30 repeat=weekdays

## passive
> Ich möchte um 19 Uhr an das Training erinnert werden
= reminder "Training" 2026-11-10T19:00
