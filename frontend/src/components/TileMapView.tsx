import { useEffect, useMemo } from 'react'
import { MapContainer, TileLayer, Polyline, CircleMarker, useMap } from 'react-leaflet'
import type { LatLngBoundsExpression, LatLngTuple } from 'leaflet'
import 'leaflet/dist/leaflet.css'
import { PathData } from '@/types'
import styles from './TileMapView.module.css'

export type TileLayerKind = 'street' | 'satellite'

interface TileMapViewProps {
  pathData: PathData
  highlightGeometry?: [number, number][]
  focusCoordinate?: [number, number]
  layer: TileLayerKind
}

const TILE_CONFIG: Record<TileLayerKind, { url: string; attribution: string }> = {
  street: {
    url: 'https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png',
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
  },
  satellite: {
    url: 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}',
    attribution: 'Tiles &copy; Esri',
  },
}

function FitBounds({ bounds }: { bounds: LatLngBoundsExpression }) {
  const map = useMap()
  useEffect(() => {
    map.fitBounds(bounds, { padding: [24, 24] })
  }, [map, bounds])
  return null
}

function FocusFly({ coordinate }: { coordinate?: LatLngTuple }) {
  const map = useMap()
  useEffect(() => {
    if (coordinate) map.flyTo(coordinate, Math.max(map.getZoom(), 17), { duration: 0.6 })
  }, [map, coordinate])
  return null
}

export default function TileMapView({ pathData, highlightGeometry, focusCoordinate, layer }: TileMapViewProps) {
  const tile = TILE_CONFIG[layer]

  const bounds = useMemo<LatLngBoundsExpression>(() => {
    const allLat: number[] = []
    const allLng: number[] = []
    for (const [lat, lng] of pathData.coordinates) {
      allLat.push(lat)
      allLng.push(lng)
    }
    for (const street of pathData.streetGeometries) {
      for (const [lat, lng] of street) {
        allLat.push(lat)
        allLng.push(lng)
      }
    }
    return [
      [Math.min(...allLat), Math.min(...allLng)],
      [Math.max(...allLat), Math.max(...allLng)],
    ]
  }, [pathData])

  const start = pathData.coordinates[0]
  const end = pathData.coordinates[pathData.coordinates.length - 1]

  return (
    <MapContainer className={styles.map} center={start} zoom={15} scrollWheelZoom zoomControl={false}>
      <TileLayer url={tile.url} attribution={tile.attribution} maxZoom={19} />
      <FitBounds bounds={bounds} />
      <FocusFly coordinate={focusCoordinate} />
      {pathData.streetGeometries.map((street, i) => (
        <Polyline key={i} positions={street} pathOptions={{ color: '#FF4A3C', opacity: 0.45, weight: 3 }} />
      ))}
      {highlightGeometry && highlightGeometry.length > 1 ? (
        <Polyline positions={highlightGeometry} pathOptions={{ color: '#FFB23D', weight: 5 }} />
      ) : (
        <Polyline positions={pathData.coordinates} pathOptions={{ color: '#FFB23D', weight: 4, opacity: 0.85 }} />
      )}
      <CircleMarker
        center={start}
        radius={7}
        pathOptions={{ color: '#0c3', fillColor: '#33E07A', fillOpacity: 1, weight: 2 }}
      />
      <CircleMarker
        center={end}
        radius={7}
        pathOptions={{ color: '#a00', fillColor: '#FF4A3C', fillOpacity: 1, weight: 2 }}
      />
    </MapContainer>
  )
}
