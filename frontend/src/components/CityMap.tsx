import { useEffect, useRef } from 'react'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'
import { useTranslation } from 'react-i18next'
import { PathData } from '@/types'
import styles from './CityMap.module.css'

function pinIcon(color: string, label: string) {
  return L.divIcon({
    className: '',
    html: `<div style="
      background:${color};color:#fff;font-weight:bold;font-size:11px;
      width:28px;height:28px;border-radius:50% 50% 50% 0;
      transform:rotate(-45deg);display:flex;align-items:center;
      justify-content:center;border:2px solid rgba(0,0,0,.35);
      box-shadow:0 2px 4px rgba(0,0,0,.4)">
      <span style="transform:rotate(45deg)">${label}</span>
    </div>`,
    iconSize: [28, 28],
    iconAnchor: [14, 28],
  })
}

function haversineM(a: L.LatLngTuple, b: L.LatLngTuple): number {
  const R = 6_371_000
  const dLat = ((b[0] - a[0]) * Math.PI) / 180
  const dLng = ((b[1] - a[1]) * Math.PI) / 180
  const h =
    Math.sin(dLat / 2) ** 2 +
    Math.cos((a[0] * Math.PI) / 180) * Math.cos((b[0] * Math.PI) / 180) * Math.sin(dLng / 2) ** 2
  return 2 * R * Math.asin(Math.sqrt(h))
}

/**
 * Split a flat coordinate list into road-following segments by cutting
 * wherever consecutive points jump more than `thresholdM` metres apart.
 * Those jumps are straight-line transfers between disconnected components
 * of the street network — they cross buildings and should not be drawn.
 */
function splitAtTransfers(coords: L.LatLngTuple[], thresholdM = 150): L.LatLngTuple[][] {
  if (coords.length === 0) return []
  const segments: L.LatLngTuple[][] = []
  let current: L.LatLngTuple[] = [coords[0]]
  for (let i = 1; i < coords.length; i++) {
    if (haversineM(coords[i - 1], coords[i]) > thresholdM) {
      segments.push(current)
      current = [coords[i]]
    } else {
      current.push(coords[i])
    }
  }
  segments.push(current)
  return segments
}

export default function CityMap({ pathData }: { pathData: PathData }) {
  const mapRef = useRef<HTMLDivElement>(null)
  const map = useRef<L.Map | null>(null)
  const { t } = useTranslation()

  useEffect(() => {
    if (!mapRef.current || !pathData?.coordinates?.length) return

    if (!map.current) {
      const [centerLat, centerLng] = pathData.coordinates[0]
      map.current = L.map(mapRef.current).setView([centerLat, centerLng], 13)
      L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '© OpenStreetMap contributors',
        maxZoom: 19,
      }).addTo(map.current)
    }

    const latlngs: L.LatLngTuple[] = pathData.coordinates.map(([lat, lng]) => [lat, lng])
    const segments = splitAtTransfers(latlngs)

    // Draw each road-following segment as its own polyline, skipping the
    // straight-line transfers between disconnected street components.
    const allBounds = L.latLngBounds(latlngs)
    for (const seg of segments) {
      L.polyline(seg, { color: '#0066cc', weight: 3, opacity: 0.8 }).addTo(map.current!)
    }

    const start = latlngs[0]
    const end = latlngs[latlngs.length - 1]
    L.marker(start, { icon: pinIcon('#22a722', t('map.startLabel')), title: t('map.startTitle') }).addTo(map.current)
    L.marker(end, { icon: pinIcon('#cc2200', t('map.endLabel')), title: t('map.endTitle') }).addTo(map.current)

    map.current.fitBounds(allBounds.pad(0.1))
  }, [pathData, t])

  return <div ref={mapRef} className={styles.map} />
}
