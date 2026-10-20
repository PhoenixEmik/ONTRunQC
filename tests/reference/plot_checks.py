"""Independent, population-aware checks for actual captured plot layers.

Different grids are measured separately. Never pretend unlike bins have a
meaningful binwise residual, or use these checks to waive strict GAM failures.
"""
import bisect
from collections import Counter
import csv
import math
import statistics


def input_rows(paths):
    for path in paths:
        with path.open() as stream:
            reader = csv.DictReader(stream, delimiter='\t')
            two_d = 'sequence_length_2d' in reader.fieldnames
            for row in reader:
                if row.get('calibration_strand_genome_template') == 'YHR174W':
                    continue
                yield (int(row['sequence_length_2d' if two_d else 'sequence_length_template']),
                       float(row['mean_qscore_2d' if two_d else 'mean_qscore_template']),
                       int(row['channel']), float(row['start_time']))


def result(metric, reference, actual, classification=None, explanation=''):
    difference = actual-reference
    return dict(metric=metric, reference=reference, ontrunqc_or_independent=actual,
                absolute_difference=abs(difference), signed_difference=difference,
                tolerance=0, status='exact_match' if difference == 0 else classification or 'unexplained_difference',
                explanation=explanation)


def checks(capture, reports, paths, cutoff):
    """Small validation fixtures may retain tuples here; runtime engine does not."""
    rows = list(input_rows(paths))
    out = []
    for plot in capture['plots']:
        family = plot['file'].rsplit('.', 1)[0]
        directory = 'combined' if plot['directory'] == 'combinedQC' else plot['directory']
        report = reports[directory]
        if directory == 'combined':
            population = rows
        else:
            index = int(directory.rsplit('_', 1)[1])-1
            population = list(input_rows([paths[index]]))
        for layer in plot['layers']:
            audit = layer.get('audit', {})
            grid = audit.get('grid')
            if grid:
                out.append(result(f'{directory}.{family}.grid_finite_conservation',
                                  audit['finite_xy'], sum(grid['counts'])))
            data = audit.get('data') or []
            if layer['stat'] not in ('StatBin', 'StatBin2d', 'StatCount'):
                continue
            for panel in plot['panels']:
                selected = panel.get('Q_cutoff', 'All reads') != 'All reads'
                subset = [r for r in population if not selected or r[1] >= cutoff]
                bins = [r for r in data if str(r['PANEL']) == str(panel['PANEL'])]
                if not bins:
                    continue
                total = sum(b.get('count', 0) for b in bins)
                key = f"{directory}.{family}.panel_{panel['PANEL']}"
                if family == 'channel_summary':
                    expected = len(set(r[2] for r in subset))
                else:
                    expected = sum(r[0]>0 for r in subset) if ('length_histogram' in family or family == 'length_vs_q') else len(subset)
                out.append(result(key+'.reference_count_conservation', expected, total))
                if 'histogram' in family:
                    length = 'length_histogram' in family
                    # R default closed='right'; fuzz at numeric boundaries is
                    # deliberately not hidden by tolerance. Unexpected mismatch
                    # stays unexplained and requires investigation.
                    boundaries = [b['xmax'] for b in bins]
                    expected_bins = [0]*len(bins)
                    outside = 0
                    for length_bp, q, _, _ in subset:
                        if length and length_bp <= 0:
                            continue
                        value = math.log10(length_bp) if length else q
                        index = bisect.bisect_left(boundaries, value)
                        if index == len(bins) or value < bins[0]['xmin']:
                            outside += 1
                        else:
                            expected_bins[index] += 1
                    out.append(result(key+'.outside_captured_bins', 0, outside))
                    out.append(result(key+'.independent_bin_L1', 0, sum(abs(n-b['count']) for n,b in zip(expected_bins,bins))))
                    native_bins = (report['comparison']['length_histogram' if length else 'quality_histogram'] if selected else
                                   report['read_length_distribution'] if length else report['quality']['histogram'])
                    native_count = sum(b['read_count'] for b in native_bins)
                    out.append(result(key+'.population_count_difference', total, native_count,
                        'intentional_population_difference', 'Native basecalled histograms exclude zero-length records; R Q histogram includes their Q.'))
                    out.append(result(key+'.bin_count_difference', len(bins), len(native_bins),
                        'different_aggregation', 'Automatic ggplot bins versus documented fixed native intervals. Count conservation is not shape equivalence.'))
        if family == 'reads_per_hour':
            layer = next(l for l in plot['layers'] if l.get('audit',{}).get('geom') == 'GeomLine')
            data = layer['audit'].get('sample') or layer['audit'].get('data') or []
            # Small hourly layers are retained completely by the bounded sample.
            if layer['rows'] <= 256:
                for group in sorted(set(d['group'] for d in data)):
                    series = [d for d in data if d['group'] == group]
                    selected = series[0]['colour'] != plot['colours'][plot['colour_labels'].index('All reads')]
                    expected = Counter(int(r[3]//3600) for r in population if not selected or r[1]>=cutoff)
                    out.append(result(f'{directory}.reads_per_hour.group_{group}.hourly_count_L1', 0,
                        sum(abs(d['y']-expected[int(d['x'])]) for d in series)))
        if family == 'gb_per_channel_overview' and plot.get('source_tiles'):
            tiles = plot['source_tiles']
            native = {c['channel']:c['total_yield_bp'] for c in report['channels']}
            selected = {c['channel']:c['qscore_selected_reads']['total_yield_bp'] for c in report['comparison']['channels']}
            out.append(result(directory+'.spatial.total_yield_cell_L1',0,
                sum(abs(t['value']-(native if t['Q_cutoff']=='All reads' else selected).get(t['channel'],0)) for t in tiles)))
            coordinates = {c['channel']:c for c in report['diagnostics']['physical_layout']['channel_coordinates']}
            out.append(result(directory+'.spatial.coordinate_L1',0,
                sum(abs(t['row']-1-coordinates[t['channel']]['row']) +
                    abs(t['col']-1-coordinates[t['channel']]['column']) for t in tiles)))
    return out


def curve_measurements(report, rows):
    """Quantify display interpolation separately from exact query statistics."""
    from bisect import bisect_right
    curve = report['comparison']['yield_by_minimum_length']['curve']
    x = [p['minimum_length_bp'] for p in curve]
    by_length = Counter(r[0] for r in rows if r[0]>0)
    total = sum(length*n for length,n in by_length.items())
    maximum_error = 0.0
    running = total
    for length,n in sorted(by_length.items()):
        i = max(0,min(len(curve)-2,bisect_right(x,length)-1))
        left,right = curve[i],curve[min(i+1,len(curve)-1)]
        span = right['minimum_length_bp']-left['minimum_length_bp']
        weight = (length-left['minimum_length_bp'])/span if span else 0
        interpolated = left['all_yield_bp']*(1-weight)+right['all_yield_bp']*weight
        maximum_error=max(maximum_error,abs(interpolated-running))
        running -= length*n
    keys=sorted(by_length)
    prefix=[0]
    for length in keys: prefix.append(prefix[-1]+length*by_length[length])
    query_error=max((abs(p['all_yield_bp']-(total-prefix[bisect.bisect_left(keys,p['minimum_length_bp'])])) for p in curve),default=0)
    return dict(maximum_exact_query_error_bp=query_error,
                maximum_display_interpolation_error_bp=maximum_error,
                definition='Maximum absolute native linear display interpolation minus exact inclusive retained yield at observed lengths; not an error in stored queries.',
                all_summary_records=len(rows),basecalled_reads=sum(r[0]>0 for r in rows),
                zero_length_records=sum(r[0]==0 for r in rows),yield_bp=total,
                maximum_15_min_bin_yield_bp=max((b['yield_bp'] for b in report['time_bins']),default=0))


def compatible_bin_checks(capture, reports, paths=None, cutoff=None):
    """Original ggplot_build rows versus explicit Rust statistical layers (#19).

    Counts/row counts are exact. Coordinates allow only 1e-12 absolute + 1e-13
    relative serialization/libm error, independent of GAM/KDE tolerances.
    """
    out=[]
    channel_populations={}
    def membership_error(bins,values):
        if not bins: return len(values),len(values)
        fuzz=1e-8*statistics.median(b["xmax"]-b["xmin"] for b in bins)
        upper=[b["xmax"]+fuzz for b in bins]
        expected=[0]*len(bins); outside=0
        for value in values:
            index=bisect.bisect_left(upper,value)
            if index==len(bins) or value<bins[0]["xmin"]-fuzz: outside+=1
            else: expected[index]+=1
        return sum(abs(n-b["count"]) for n,b in zip(expected,bins)),outside
    metrics={'Number of bases per channel':'total_yield_bp','Number of reads per channel':'read_count',
             'Mean read length per channel':'mean_read_length_bp','Median read length per channel':'median_read_length_bp'}
    for plot in capture['plots']:
        family=plot['file'].rsplit('.',1)[0]
        directory='combined' if plot['directory']=='combinedQC' else plot['directory']
        model=reports[directory].get('minionqc_plot_bins')
        if not model: continue
        for layer in plot['layers']:
            stat=layer.get('stat')
            rows=layer.get('audit',{}).get('data') or []
            if stat not in ('StatBin','StatBin2d'): continue
            panels=plot.get('panels') or [dict(PANEL=1)]
            for panel in panels:
                native=None; intentional=False
                selected=panel.get('Q_cutoff','All reads')!='All reads'
                population='q_selected' if selected else 'all'
                if 'length_histogram' in family: native=model['length'][population]
                elif 'q_histogram' in family: native=model['quality'][population]
                elif family=='channel_summary':
                    metric=metrics[panel['variable']]
                    native=next(v['histogram'][population] for v in model['channels'] if v['metric']==metric)
                    intentional=metric=='read_count'
                elif family=='length_vs_q' and stat=='StatBin2d': native=model['length_vs_q']['cells']
                if native is None: continue
                reference=[r for r in rows if str(r['PANEL'])==str(panel['PANEL'])]
                keys=['x','xmin','xmax']+(['y','ymin','ymax'] if stat=='StatBin2d' else [])
                reference=sorted(reference,key=lambda v:tuple(v[k] for k in ('x','y') if k in keys))
                native=sorted(native,key=lambda v:tuple(v[k] for k in ('x','y') if k in keys))
                errors=[abs(a[k]-b[k]) for a,b in zip(reference,native) for k in keys]
                failed=sum(abs(a[k]-b[k])>1e-12+1e-13*abs(a[k]) for a,b in zip(reference,native) for k in keys)
                l1=sum(abs(a['count']-b['count']) for a,b in zip(reference,native))
                failed+=int(len(reference)!=len(native))
                ok=not failed and l1==0
                name=f'{directory}.{family}.panel_{panel["PANEL"]}'
                proof=None
                if intentional and paths is not None:
                    if directory not in channel_populations:
                        chosen=paths if directory=='combined' else [paths[int(directory.rsplit('_',1)[1])-1]]
                        all_counts=Counter();q_counts=Counter()
                        for _,q,ch,_ in input_rows(chosen):
                            all_counts[ch]+=1
                            if q>=cutoff: q_counts[ch]+=1
                        channel_populations[directory]=(all_counts,q_counts)
                    values=list(channel_populations[directory][int(selected)].values())
                    native_l1,native_outside=membership_error(native,values)
                    reference_l1,reference_outside=membership_error(reference,[n*(n+1)//2 for n in values])
                    proof=dict(native_true_count_L1=native_l1,native_outside=native_outside,
                        reference_triangular_count_L1=reference_l1,reference_outside=reference_outside)
                status=('intentional_upstream_defect_difference' if proof and not any(proof.values()) else
                        'unexplained_difference' if proof else 'unverified_upstream_count_difference') if intentional else (
                        'exact_match' if ok and not any(errors) else 'within_tolerance' if ok else 'unexplained_difference')
                out.append(dict(metric=name,reference_rows=len(reference),actual_rows=len(native),
                    reference_population=sum(r['count'] for r in reference),actual_population=sum(r['count'] for r in native),
                    count_L1=l1,maximum_absolute_boundary_error=max(errors,default=0),
                    failed_coordinates=failed,tolerance=dict(count=0,coordinate_absolute=1e-12,coordinate_relative=1e-13),
                    status=status,upstream_defect_proof=proof,
                    explanation='True per-channel counts replace upstream triangular n(n+1)/2; both values AND resulting limits may differ.' if intentional else 'Original built rows compared with corresponding Rust bins/cells; no coarse-histogram reconstruction.',
                    first_reference=reference[:1],first_actual=native[:1],last_reference=reference[-1:],last_actual=native[-1:]))
    return out


def compatible_time_checks(capture,reports):
    """Exact hours/terminals; separately measured full-reference envelope error.

    A bounded display is never classified as point-for-point agreement. R's
    arbitrary tied-read ordering is reported separately from same-time maxima.
    """
    out=[]
    def population(plot,panel,colour):
        info=next(p for p in plot['panels'] if str(p['PANEL'])==str(panel))
        q=info.get('Q_cutoff')
        if q is None: q=plot['colour_labels'][plot['colours'].index(colour)]
        return 'all' if q=='All reads' else 'q_selected'
    for plot in capture['plots']:
        family=plot['file'].rsplit('.',1)[0]
        if family not in ('reads_per_hour','yield_over_time'):continue
        scope='combined' if plot['directory']=='combinedQC' else plot['directory']
        report=reports[scope]
        for layer in plot['layers']:
            audit=layer.get('audit',{})
            if audit.get('geom')!='GeomLine':continue
            if family=='reads_per_hour':
                data=audit.get('data')
                if data is None:raise RuntimeError('Hourly capture was not complete; refusing sample-only verification')
                for group in sorted(set(d['group'] for d in data)):
                    rows=[d for d in data if d['group']==group]
                    key=population(plot,rows[0]['PANEL'],rows[0]['colour'])
                    # Original script only has per-source hourly figures.
                    model=report['minionqc_time_series'][key]
                    actual=[(h['hour'],h['read_count']) for h in model['hourly']]
                    expected=[(d['x'],d['y']) for d in rows]
                    error=sum(abs(a[0]-b[0])+abs(a[1]-b[1]) for a,b in zip(actual,expected))
                    out.append(dict(metric=f'{scope}.{family}.{key}',reference_points=len(expected),native_points=len(actual),hour_count_L1=error,
                        reference_observations=sum(d['y'] for d in rows),native_observations=model['observations'],
                        status='exact_match' if len(expected)==len(actual) and error==0 and sum(d['y'] for d in rows)==model['observations'] else 'unexplained_difference'))
            else:
                groups=audit.get('temporal_comparisons') or []
                if isinstance(groups,dict):groups=list(groups.values())
                if not groups:raise RuntimeError('Missing actual R temporal comparisons')
                for group in groups:
                    key=population(plot,group['panel'],group['colour'])
                    source=scope
                    if scope=='combined':
                        source=plot['colour_labels'][plot['colours'].index(group['colour'])]
                        model=next(r for r in report['collection']['runs'] if r['source']['source_id']==source)['minionqc_time_series'][key]
                    else:model=report['minionqc_time_series'][key]
                    candidate=source+':'+key
                    c=next(c for c in group['comparisons'] if c['candidate']==candidate)
                    curve=model['cumulative'];error=c['maximum_envelope_error_bp']
                    original_error=c['maximum_original_polyline_error_bp']
                    tie_bound=curve.get('maximum_tied_yield_bp',0)
                    # Only GB-to-bp serialization/arithmetic roundoff; count and
                    # terminal integer tolerances are zero. No GAM tolerance used.
                    rounding=1e-5
                    passed=(c['reference_observations']==model['observations'] and c['maximum_endpoint_coordinate_error_hours']==0 and c['unmatched_native_coordinates']==0 and c['maximum_coordinate_error_hours']==0 and
                        c['reference_terminal_yield_bp']==model['total_yield_bp']==c['native_terminal_yield_bp'] and error<=curve['yield_error_bound_bp']+rounding and original_error<=curve['yield_error_bound_bp']+tie_bound+rounding)
                    out.append(dict(metric=f'{scope}.{family}.{source}.{key}',**{k:v for k,v in c.items() if k!='samples'},
                        omitted_points=curve['omitted_points'],yield_error_bound_bp=curve['yield_error_bound_bp'],rounding_tolerance_bp=rounding,
                        measured_native_order_error_bp=curve['maximum_linear_interpolation_error_bp'],
                        original_polyline_error_bound_bp=curve['yield_error_bound_bp']+tie_bound,
                        original_polyline_status=('exact_match' if original_error==0 else 'rounding_match' if original_error<=rounding else 'bounded_display_difference' if original_error<=curve['yield_error_bound_bp']+rounding else 'bounded_display_and_tie_order_difference'),
                        status=('exact_time_envelope' if error==0 else 'rounding_time_envelope' if error<=rounding else 'bounded_display_difference') if passed else 'unexplained_difference',
                        explanation='All original R timestamp maxima AND actual GeomLine left/right limits evaluated. Exact endpoints/counts; explicit simplification bound plus separately disclosed tied-order bound. Not point-for-point parity.'))
    return out
