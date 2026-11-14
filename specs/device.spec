# The phone itself, moved from the app's old Kotlin rules into the core. Clock: Tuesday 2026-11-10, 16:00.

## volume to a percentage
> Lautstärke auf 40 Prozent
< 40 Prozent
= device "percent":40
= device "ring":false

## volume with the percent sign, ring volume
> Stell die Klingellautstärke auf 70%
< Klingellautstärke
= device "ring":true
= device "percent":70

## volume in words
> Lautstärke auf fünfzig Prozent
= device "percent":50

## louder, quieter
> Mach lauter
= device "change":"up"
> etwas leiser
= device "change":"down"
= devices 2

## maximum and mute
> Lautstärke maximal
= device "percent":100
> Lautstärke stumm
= device "percent":0

## a level that is not one is not a volume command
> Lautstärke auf irgendwas
= devices 0

## brightness
> Helligkeit auf 30 Prozent
< 30 Prozent
= device "brightness"
> mach den Bildschirm heller
= device "change":"up"
> Helligkeit automatisch
< Automatische Helligkeit
= device "auto"

## the permission is missing: the reply says what to allow
@device denied
> Helligkeit auf 30 Prozent
< Systemeinstellungen
= kind failed
> Lautstärke auf 10 Prozent
< Nicht stören

## Android refuses
@device failed
> Lautstärke auf 10 Prozent
< nicht übernommen

## sound mode
> Stell das Handy auf lautlos
< lautlos
= device "silent"
> Handy auf Vibration
= device "vibrate"
> Stell das Handy auf laut
= device "ring"

## do not disturb
> Nicht stören an
< Nicht stören ist an
= device "on":true
> Schalte nicht stören aus
< ist aus
= device "on":false
> Deaktiviere nicht stören
= device "on":false

## undo restores the setting
> Lautstärke auf 40 Prozent
> rückgängig
< rückgängig gemacht
= restored 1

## web search keeps the words as said
> Suche im Internet nach Rezepte mit Kürbis
< „Rezepte mit Kürbis“
= device "query":"Rezepte mit Kürbis"

## a search that does not say where is not ours
> Suche nach meinem Schlüssel
= devices 0

## what is playing
@playing "Bohemian Rhapsody" Queen
> Was läuft gerade?
< Bohemian Rhapsody von Queen

## no player
> Was läuft gerade
< keinen Player
= kind failed

## notifications
@notice "Messenger" "Anna" "Bin gleich da"
@notice "Mail" "Rechnung" "Ihre Rechnung"
> Lies meine Benachrichtigungen vor
< Anna: Bin gleich da
< Rechnung: Ihre Rechnung
> Lies die Nachrichten von Messenger vor
< Anna
<! Rechnung

## no access to notifications
@device denied
> Lies meine Benachrichtigungen vor
< Zugriff auf Benachrichtigungen

## reply to the only conversation, confirmed
@notice "Messenger" "Anna" "Kommst du?" reply
> Antworte Anna mit ich komme doch später
< An „Anna“ antworten: „ich komme doch später“?
= kind question
> ja
< übergeben
= replied "ich komme doch später"

## reply declined
@notice "Messenger" "Anna" "Kommst du?" reply
> Antworte Anna mit ok
> nein
< abgebrochen

## reply with several conversations: pick one by number
@notice "Messenger" "Anna" "Hallo" reply
@notice "Messenger" "Ben" "Hi" reply
> Antworte auf Messenger mit bis morgen
< Welche Unterhaltung?
< 2. Ben
> die zweite
< Die Nummer
> 2
< An „Ben“ antworten
> ja
= replied "bis morgen"

## nothing to answer
@notice "Mail" "Rechnung" "Ihre Rechnung"
> Antworte Mail mit danke
< keine passende Benachrichtigung
= kind failed

# ---- stopwatch ----

## start, read, stop, reset
> Starte die Stoppuhr
< läuft
= kept stopwatch.started
@advance 75s
> Wie lange läuft die Stoppuhr?
< 00:01:15
@advance 15s
> Stoppe die Stoppuhr
< 00:01:30, angehalten
= kept !stopwatch.started
@advance 60s
> Stoppuhr
< 00:01:30
> Starte die Stoppuhr
@advance 10s
> Stoppuhr anhalten
< 00:01:40
> Setze die Stoppuhr zurück
< 00:00:00

## a stop is not a cancel
> Stoppe die Stoppuhr
< Stoppuhr
<! abgebrochen

# ---- converting units ----

## miles in kilometres, in words
> Fünf Meilen in Kilometer
< 5 Meilen sind 8,04672 Kilometer

## temperature with "Grad"
> Wie viel sind 100 Grad Celsius in Fahrenheit
< 212 Grad Fahrenheit

## decimal comma, negative
> -40 Fahrenheit in Celsius
< -40
> rechne 2,5 kg in Gramm um
< 2,5 Kilogramm sind 2500 Gramm

## multi-word unit
> 100 km/h in Meilen pro Stunde
< 62,137119 mph

## units that do not fit
> 5 Meter in Kilogramm
< nicht ineinander umrechnen

## not a conversion: lists still get it
> Füge zwei Eier zur Einkaufsliste hinzu
<! umrechnen
= count item 1

# ---- alarms ----

## a repeating alarm on weekdays
> Weck mich werktags um 6:30
< werktags um 6:30
= alarm 2026-11-11T06:30 days=1,2,3,4,5

## daily
> Stell einen Wecker täglich um 7 Uhr
= alarm 2026-11-11T07:00 days=1,2,3,4,5,6,7

## dismiss the next alarm, confirmed
> Schalte den Wecker aus
< verwerfen?
= kind question
> ja
< gebeten
= device dismiss_alarm

## dismiss declined
> Verwirf den nächsten Wecker
> nein
< abgebrochen
= devices 0

# ---- help ----

## commands
> Befehle
< Zum Beispiel
< Stoppuhr

# ---- English ----

## volume, brightness, dnd, search, stopwatch, convert
@lang en
> Volume to 40 percent
< 40 percent
> make it brighter
= device "brightness"
> turn on do not disturb
< Do Not Disturb is on
> search the web for pumpkin soup
< pumpkin soup
> start the stopwatch
< running
> five miles in kilometres
< 5 miles is 8.04672 kilometres
> dismiss the next alarm
< dismiss the next alarm?
> commands
< For example

## an answer that is neither yes nor no is a new request
> Verwirf den nächsten Wecker
> Starte die Stoppuhr
< Stoppuhr
= devices 0
