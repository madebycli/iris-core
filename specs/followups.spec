# Found in the phone-test dry run before build 122: questions that swallowed the next request,
# sentences that started the wrong thing, and names said only in part. Clock: Tuesday 2026-11-10, 16:00.

## K1 a new request ends an open question (same skill)
> Erinnere mich morgen um 8 an den Zahnarzt
> Erinnere mich
< Wann
> Welche Erinnerungen habe ich?
< Zahnarzt
<! Wann soll ich
> Verschiebe die Erinnerung Zahnarzt auf 9 Uhr
= reminder "Zahnarzt" 2026-11-11T09:00
= count reminder 1

## K1 a new request ends an open question (other skill)
> Erinnere mich
> Setz Milch auf die Einkaufsliste
< Milch steht auf der Liste Einkauf
= item "Milch" list=Einkauf
> Wie spät ist es?
< 16:00
<! Wann

## K1 answers still answer
> Erinnere mich
> Müll rausbringen
< Wann soll ich dich an „Müll rausbringen“ erinnern?
> morgen um 7
= reminder "Müll rausbringen" 2026-11-11T07:00

## K1 a reminder title that names a list stays a title
> Erinnere mich morgen um 8
< heißen
> Einkaufsliste mitnehmen
= reminder "Einkaufsliste mitnehmen" 2026-11-11T08:00
= count item 0

## K1 "ach, vergiss es" cancels
> Erinnere mich
> Ach, vergiss es
< abgebrochen
> Timer
> ach nee, lass mal
< abgebrochen

## K2 asking for the running timers
> Stell einen Timer auf 10 Minuten
> Stell einen Timer für die Nudeln auf 8 Minuten
> Zeig mir meine aktiven Timer
< Nudeln: noch 8 Minuten
<! Wie lange soll
> Wie lange läuft der Nudeltimer noch?
< noch 8 Minuten
> Welche Timer habe ich?
< Nudeln
= count timer 2

## K2 timer while a timer question is open
> Timer
< Wie lange
> Welche Timer laufen?
< Es läuft kein Timer

## timer pause, resume, extend, restart
> Stell einen Timer für die Nudeln auf 8 Minuten
@advance 120s
> Pausiere den Nudeltimer
< angehalten
< 6 Minuten
@advance 600s
> Wie lange läuft der Timer noch?
< 6 Minuten
> Setz den Timer fort
< läuft weiter
@advance 60s
> Verlängere den Timer um 5 Minuten
< 10 Minuten
> Starte den Timer neu
< 8 Minuten

## timer control with two timers asks which
> Stell einen Timer für die Nudeln auf 8 Minuten
> Stell einen Timer für den Tee auf 3 Minuten
> Pausiere den Timer
< Welchen Timer
> Tee
< Tee
< angehalten

## K3 reading notes in every phrasing
> Notiere die Paketnummer 12345
> Was habe ich notiert?
< Paketnummer 12345
= count note 1
> Was hab ich mir alles notiert
< Paketnummer
> Was steht in meinen Notizen?
< Paketnummer
= count note 1

## K4 deleting an alarm does not set one
> Lösche den Wecker für 15 Uhr
< 15:00
< ?
= alarms 0
> ja
= device dismiss_alarm
= device "hour":15
= alarms 0

## K4 delete my alarm, without a time
> Lösche meinen Wecker
< nächsten Wecker
> nein
< abgebrochen
= devices 0

## K5 "auf die Liste schreiben" is not a list called Schreiben
> Ich möchte etwas auf die Liste schreiben
< Auf welche Liste?
> Einkaufsliste
< Was soll auf die Liste Einkauf?
> Milch
= item "Milch" list=Einkauf

## K6 two actions in one sentence
@item "Eier" Einkauf
> Füge Käse zur Einkaufsliste hinzu und entferne Eier
< Käse
< Eier
= item "Käse" list=Einkauf
= count item 1

## K6 two skills in one sentence
> Setz Milch auf die Einkaufsliste und stell einen Timer auf 5 Minuten
= item "Milch" list=Einkauf
= count timer 1

## K6 "und" inside an entry does not split
> Setz Eier und Butter auf die Einkaufsliste
= count item 2
> Erinnere mich morgen um 8 an Brot und Butter
= reminder "Brot und Butter" 2026-11-11T08:00

## K7 a reminder named in part
> Erinnere mich morgen um 9 an Müll rausbringen
> Erinnere mich morgen um 10 an den Zahnarzt
> Lösche die Erinnerung Müll
< „Müll rausbringen“
= count reminder 1
> Verschiebe die Erinnerung Zahn auf 11 Uhr
= reminder "Zahnarzt" 2026-11-11T11:00

## K7 a note named in part
> Notiere die Paketnummer 12345
> Notiere Oma anrufen
> Lösche die Notiz mit der Paketnummer
< gelöscht
= count note 1

## K7 a part that fits two is asked
> Erinnere mich morgen um 9 an Müll rausbringen
> Erinnere mich morgen um 10 an Müll trennen
> Lösche die Erinnerung Müll
< Welche Erinnerung
> trennen
< „Müll trennen“
= count reminder 1

## K9 "nein danke" after "Noch etwas?" ends without "abgebrochen"
> Setz was auf die Einkaufsliste
> Milch
< Noch etwas?
> nein danke
<! abgebrochen
= item "Milch" list=Einkauf

## K9 the card of all lists has a title
> Setz Milch auf die Einkaufsliste
> Welche Listen habe ich?
< Einkauf: Milch

## K8 writing a new message is said to be impossible, not sent to school
> Schreib Anna eine Nachricht
< kann ich nicht schreiben
= kind failed
> Schick eine Nachricht an Mama
< Antworte
> Schreib eine Notiz: Anna anrufen
= count note 1

## K9 "say that again" after another part of the app answered
> Wie spät ist es?
< 16:00
