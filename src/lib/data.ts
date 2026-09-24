import type { MediaItem } from '$lib/types';

const image = (id: string, width: number, height: number) =>
	`https://images.unsplash.com/${id}?auto=format&fit=crop&crop=faces,center&q=82&w=${width}&h=${height}`;

export const media: MediaItem[] = [
	{
		id: 'the-last-signal',
		title: 'The Last Signal',
		kind: 'movie',
		year: 2024,
		genres: ['Sci-fi', 'Drama'],
		rating: '8.1',
		runtime: '1h 52m',
		poster: image('photo-1519681393784-d120267933ba', 520, 780),
		backdrop: image('photo-1446776811953-b23d57bd21aa', 1800, 1000),
		synopsis: 'A radio astronomer hears a pattern inside the last transmission from a vanished expedition and has one night to decide whether to answer.',
		match: '98%',
		progress: 0.62,
		progressLabel: '41m left'
	},
	{
		id: 'north-of-ordinary',
		title: 'North of Ordinary',
		kind: 'series',
		year: 2023,
		genres: ['Drama', 'Mystery'],
		rating: '8.7',
		runtime: '3 seasons',
		poster: image('photo-1485846234645-a62644f84728', 520, 780),
		backdrop: image('photo-1485846234645-a62644f84728', 1500, 820),
		synopsis: 'A quiet coastal town keeps a careful record of every visitor. One missing page pulls a local archivist into a story that has been waiting for her.',
		match: '96%',
		progress: 0.3,
		progressLabel: 'S2 E3 · 28m left',
		seasons: 3,
		episodes: [
			{ id: 'north-ep-1', title: 'The Ledger', number: 1, duration: '48m', summary: 'Mara finds a page that should not exist in the town archive.', thumbnail: image('photo-1511497584788-876760111969', 520, 292), progress: 1 },
			{ id: 'north-ep-2', title: 'Low Tide', number: 2, duration: '51m', summary: 'A low tide exposes more than the old pier was built to hide.', thumbnail: image('photo-1500534623283-312aade485b7', 520, 292), progress: 1 },
			{ id: 'north-ep-3', title: 'The Visitor', number: 3, duration: '47m', summary: 'The archive receives a visitor who knows the missing page by heart.', thumbnail: image('photo-1524712245354-2c4e5e7121c0', 520, 292), progress: 0.42 },
			{ id: 'north-ep-4', title: 'A Narrow Room', number: 4, duration: '54m', summary: 'Mara follows a trail of rooms that are absent from every map.', thumbnail: image('photo-1497366811353-6870744d04b2', 520, 292) }
		]
	},
	{
		id: 'quiet-forms',
		title: 'Quiet Forms',
		kind: 'movie',
		year: 2022,
		genres: ['Documentary', 'Art'],
		rating: '7.9',
		runtime: '1h 38m',
		poster: image('photo-1513364776144-60967b0f800f', 520, 780),
		backdrop: image('photo-1513364776144-60967b0f800f', 1500, 820),
		synopsis: 'Five artists make room for silence, material and repetition in a portrait of process over performance.',
		match: '91%'
	},
	{
		id: 'the-deep-hour',
		title: 'The Deep Hour',
		kind: 'series',
		year: 2024,
		genres: ['Thriller', 'Crime'],
		rating: '8.3',
		runtime: '1 season',
		poster: image('photo-1518709268805-4e9042af9f23', 520, 780),
		backdrop: image('photo-1518709268805-4e9042af9f23', 1500, 820),
		synopsis: 'A late-night dispatcher notices that a string of unrelated calls share the same impossible timestamp.',
		match: '94%',
		seasons: 1,
		episodes: [
			{ id: 'deep-ep-1', title: '00:17', number: 1, duration: '46m', summary: 'A call comes through from a street that was demolished ten years ago.', thumbnail: image('photo-1500534623283-312aade485b7', 520, 292) },
			{ id: 'deep-ep-2', title: 'The Quiet Line', number: 2, duration: '49m', summary: 'The same voice begins appearing across different channels.', thumbnail: image('photo-1511497584788-876760111969', 520, 292) }
		]
	},
	{
		id: 'after-the-rain',
		title: 'After the Rain',
		kind: 'movie',
		year: 2021,
		genres: ['Romance', 'Drama'],
		rating: '7.6',
		runtime: '2h 06m',
		poster: image('photo-1490750967868-88aa4486c946', 520, 780),
		backdrop: image('photo-1490750967868-88aa4486c946', 1500, 820),
		synopsis: 'Two strangers share a borrowed camera over one summer and discover the life that exists between the frames.',
		match: '88%'
	},
	{
		id: 'small-worlds',
		title: 'Small Worlds',
		kind: 'series',
		year: 2020,
		genres: ['Comedy', 'Drama'],
		rating: '8.0',
		runtime: '2 seasons',
		poster: image('photo-1529156069898-49953e39b3ac', 520, 780),
		backdrop: image('photo-1529156069898-49953e39b3ac', 1500, 820),
		synopsis: 'A group of neighbors build a surprisingly serious radio station from the back room of a corner shop.',
		match: '90%',
		seasons: 2,
		episodes: [
			{ id: 'small-ep-1', title: 'Test Tone', number: 1, duration: '29m', summary: 'The station goes live before anyone has agreed on a name.', thumbnail: image('photo-1492684223066-81342ee5ff30', 520, 292) },
			{ id: 'small-ep-2', title: 'Dead Air', number: 2, duration: '31m', summary: 'A technical failure turns into the most listened-to broadcast in town.', thumbnail: image('photo-1529156069898-49953e39b3ac', 520, 292) }
		]
	},
	{
		id: 'a-map-of-winter',
		title: 'A Map of Winter',
		kind: 'movie',
		year: 2024,
		genres: ['Adventure', 'Drama'],
		rating: '8.5',
		runtime: '2h 14m',
		poster: image('photo-1464822759023-fed622ff2c3b', 520, 780),
		backdrop: image('photo-1464822759023-fed622ff2c3b', 1500, 820),
		synopsis: 'A cartographer returns to a frozen valley to finish the map her father left behind and finds the route has changed.',
		match: '97%'
	},
	{
		id: 'the-still-room',
		title: 'The Still Room',
		kind: 'movie',
		year: 2019,
		genres: ['Mystery', 'Drama'],
		rating: '7.8',
		runtime: '1h 45m',
		poster: image('photo-1497366754035-f200968a6e72', 520, 780),
		backdrop: image('photo-1497366754035-f200968a6e72', 1500, 820),
		synopsis: 'A restorer is hired to catalogue a house where every room has been preserved exactly as it was on one winter morning.',
		match: '86%'
	},
	{
		id: 'current-weather',
		title: 'Current Weather',
		kind: 'series',
		year: 2022,
		genres: ['Drama', 'Slice of life'],
		rating: '8.2',
		runtime: '4 seasons',
		poster: image('photo-1500534314209-a25ddb2bd429', 520, 780),
		backdrop: image('photo-1500534314209-a25ddb2bd429', 1500, 820),
		synopsis: 'At a small weather station, daily forecasts become a way for four people to say what they cannot say directly.',
		match: '93%',
		seasons: 4,
		episodes: [
			{ id: 'weather-ep-1', title: 'Pressure', number: 1, duration: '42m', summary: 'A pressure system stalls over the coast and changes everyone’s plans.', thumbnail: image('photo-1500534314209-a25ddb2bd429', 520, 292) },
			{ id: 'weather-ep-2', title: 'Visibility', number: 2, duration: '44m', summary: 'A clear morning reveals a message written across the old runway.', thumbnail: image('photo-1464822759023-fed622ff2c3b', 520, 292) }
		]
	},
	{
		id: 'night-shift',
		title: 'Night Shift',
		kind: 'movie',
		year: 2018,
		genres: ['Drama', 'Thriller'],
		rating: '7.5',
		runtime: '1h 57m',
		poster: image('photo-1519608487953-e999c86e7455', 520, 780),
		backdrop: image('photo-1519608487953-e999c86e7455', 1500, 820),
		synopsis: 'Over one overnight shift, a hospital porter follows a patient’s unfinished list through the quiet parts of the building.',
		match: '84%'
	},
	{
		id: 'the-open-sea',
		title: 'The Open Sea',
		kind: 'movie',
		year: 2017,
		genres: ['Documentary', 'Nature'],
		rating: '8.4',
		runtime: '1h 31m',
		poster: image('photo-1507525428034-b723cf961d3e', 520, 780),
		backdrop: image('photo-1507525428034-b723cf961d3e', 1500, 820),
		synopsis: 'A patient, observational journey with the crews who work beyond the horizon and the communities waiting on shore.',
		match: '95%'
	},
	{
		id: 'line-by-line',
		title: 'Line by Line',
		kind: 'series',
		year: 2021,
		genres: ['Documentary', 'Culture'],
		rating: '7.7',
		runtime: '2 seasons',
		poster: image('photo-1455390582262-044cdead277a', 520, 780),
		backdrop: image('photo-1455390582262-044cdead277a', 1500, 820),
		synopsis: 'Writers, printers and binders talk about the physical decisions hiding inside a finished book.',
		match: '89%',
		seasons: 2,
		episodes: [
			{ id: 'line-ep-1', title: 'Margins', number: 1, duration: '38m', summary: 'A margin is never empty: three designers explain what it makes possible.', thumbnail: image('photo-1455390582262-044cdead277a', 520, 292) },
			{ id: 'line-ep-2', title: 'Weight', number: 2, duration: '41m', summary: 'Paper changes the way a sentence arrives in the hand.', thumbnail: image('photo-1513364776144-60967b0f800f', 520, 292) }
		]
	}
];

export const featured = media[0];
export const continueWatching = [media[0], media[1], media[3], media[5]];
export const recentlyAdded = [media[2], media[6], media[8], media[10]];

export function getMedia(slug: string) {
	return media.find((item) => item.id === slug);
}
