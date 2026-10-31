<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use PHPUnit\Framework\TestCase;

/**
 * The extension's public surface against 1.0's, captured by tools/capture-v1-fixtures.php
 * into fixtures/v1/surface.json. The surface is rebuilt here from Reflection with that tool's
 * code path, and must equal the fixture after the edits declared in `expected()`, no others.
 */
final class SurfaceTest extends TestCase
{
    private const CLASSES = [
        'HyperUuid', 'Uuid', 'Namespaces', 'UuidLayout', 'UuidVariant',
        'CounterExhaustedException', 'RandomSourceException', 'TimestampOutOfRangeException',
    ];

    public function testSurfaceMatchesV1(): void
    {
        $expected = self::expected();
        $actual = self::normalise(self::surface());
        $differences = self::differences($expected, $actual);
        self::assertSame([], $differences, "the surface differs from 1.0's:\n" . implode("\n", $differences));
        self::assertSame($expected, $actual);
    }

    /**
     * 1.0's `HyperUuid` and `Namespaces` had private constructors, which surface.json (public
     * members only) doesn't record.
     */
    public function testStaticClassesAreNotInstantiable(): void
    {
        foreach (['HyperUuid', 'Namespaces'] as $short) {
            $class = 'HyperUuid\\' . $short;
            self::assertFalse((new \ReflectionClass($class))->isInstantiable(), $class);
            self::assertTrue((new \ReflectionMethod($class, '__construct'))->isPrivate(), $class);
            try {
                new $class();
                self::fail("{$class} was instantiated");
            } catch (\Error $e) {
                self::assertSame(\Error::class, $e::class);
                self::assertSame("Call to private {$class}::__construct() from scope " . self::class, $e->getMessage());
            }
        }
    }

    /** surface.json with the declared edits applied: these and nothing else. */
    private static function expected(): array
    {
        $path = __DIR__ . '/fixtures/v1/surface.json';
        $surface = json_decode((string) file_get_contents($path), true, 512, JSON_THROW_ON_ERROR);

        // Edit 1: HyperUuid::isAvailable() is removed (the capture already left it out of the
        // methods and recorded it here). The extension is either loaded or the classes don't
        // exist, so there is nothing left for it to report.
        self::assertSame(['HyperUuid::isAvailable'], $surface['removed']);
        unset($surface['removed']);

        // Edit 2: Uuid's serialization is explicit (1.0 had PHP's default serialization of
        // its readonly property; the payload is unchanged, serialized.txt pins it).
        $surface['Uuid']['methods']['__serialize'] = [
            'static' => false,
            'params' => [],
            'return' => 'array',
        ];
        $surface['Uuid']['methods']['__unserialize'] = [
            'static' => false,
            'params' => [['name' => 'data', 'type' => 'array', 'optional' => false, 'default' => null]],
            'return' => 'void',
        ];
        ksort($surface['Uuid']['methods']);

        return self::normalise($surface);
    }

    /** The surface of the loaded extension, built exactly as the capture tool built 1.0's. */
    private static function surface(): array
    {
        $surface = [];
        foreach (self::CLASSES as $short) {
            $rc = new \ReflectionClass('HyperUuid\\' . $short);
            $entry = [
                'kind' => $rc->isEnum() ? 'enum' : ($rc->isInterface() ? 'interface' : 'class'),
                'final' => $rc->isFinal(),
                'readonly' => $rc->isReadOnly(),
                'interfaces' => $rc->getInterfaceNames(),
                'parent' => $rc->getParentClass() ? $rc->getParentClass()->getName() : null,
                'constants' => [],
                'cases' => [],
                'methods' => [],
            ];
            sort($entry['interfaces']);
            foreach ($rc->getReflectionConstants() as $const) {
                if ($const->isEnumCase()) {
                    $case = $const->getValue();
                    $entry['cases'][$const->getName()] = $case instanceof \BackedEnum ? $case->value : null;
                } else {
                    $entry['constants'][$const->getName()] = $const->getValue();
                }
            }
            foreach ($rc->getMethods() as $m) {
                if ($m->getDeclaringClass()->getName() !== $rc->getName() || !$m->isPublic()) {
                    continue;
                }
                $params = [];
                foreach ($m->getParameters() as $p) {
                    $default = null;
                    if ($p->isDefaultValueAvailable()) {
                        $default = $p->isDefaultValueConstant()
                            ? $p->getDefaultValueConstantName()
                            : var_export($p->getDefaultValue(), true);
                    }
                    $params[] = [
                        'name' => $p->getName(),
                        'type' => $p->getType() === null ? null : (string) $p->getType(),
                        'optional' => $p->isOptional(),
                        'default' => $default,
                    ];
                }
                $entry['methods'][$m->getName()] = [
                    'static' => $m->isStatic(),
                    'params' => $params,
                    'return' => $m->getReturnType() === null ? null : (string) $m->getReturnType(),
                ];
            }
            ksort($entry['constants']);
            ksort($entry['methods']);
            $surface[$short] = $entry;
        }
        return $surface;
    }

    /** Both sides through the capture's JSON encoding, so they compare as the same types. */
    private static function normalise(array $surface): array
    {
        $flags = JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR;
        return json_decode(json_encode($surface, $flags), true, 512, JSON_THROW_ON_ERROR);
    }

    /**
     * Each difference, named: a class or member added or removed, or a member that changed.
     *
     * @return list<string>
     */
    private static function differences(array $expected, array $actual): array
    {
        $out = [];
        foreach (array_diff_key($expected, $actual) as $class => $_) {
            $out[] = "{$class}: removed";
        }
        foreach (array_diff_key($actual, $expected) as $class => $_) {
            $out[] = "{$class}: added";
        }
        foreach (array_intersect_key($expected, $actual) as $class => $want) {
            $have = $actual[$class];
            foreach (['kind', 'final', 'readonly', 'interfaces', 'parent'] as $key) {
                if ($want[$key] !== $have[$key]) {
                    $out[] = "{$class}: {$key} " . self::show($want[$key]) . ' became ' . self::show($have[$key]);
                }
            }
            foreach (['constants' => '::', 'cases' => '::', 'methods' => '::'] as $key => $sep) {
                $suffix = $key === 'methods' ? '()' : '';
                foreach (array_diff_key($want[$key], $have[$key]) as $name => $_) {
                    $out[] = "{$class}{$sep}{$name}{$suffix}: removed";
                }
                foreach (array_diff_key($have[$key], $want[$key]) as $name => $_) {
                    $out[] = "{$class}{$sep}{$name}{$suffix}: added";
                }
                foreach (array_intersect_key($want[$key], $have[$key]) as $name => $member) {
                    if ($member !== $have[$key][$name]) {
                        $out[] = "{$class}{$sep}{$name}{$suffix}: " . self::show($member)
                            . ' became ' . self::show($have[$key][$name]);
                    }
                }
            }
        }
        return $out;
    }

    private static function show(mixed $value): string
    {
        return json_encode($value, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
    }
}
