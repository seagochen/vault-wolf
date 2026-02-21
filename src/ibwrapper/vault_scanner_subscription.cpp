//
// Created by user on 5/9/25.
//


#include "ScannerSubscription.h"
#include "ibwrapper/vault_scanner_subscription.h"

ScannerSubscription ScannerSubscriptionSamples::HotUSStkByVolume()
{
    //! [hotusvolume]
    //Hot US stocks by volume
    ScannerSubscription scanSub;
    scanSub.instrument = "STK";
    scanSub.locationCode = "STK.US.MAJOR";
    scanSub.scanCode = "HOT_BY_VOLUME";
    //! [hotusvolume]
    return scanSub;
}

ScannerSubscription ScannerSubscriptionSamples::TopPercentGainersIbis()
{
    //! [toppercentgaineribis]
    //Top % gainers at IBIS
    ScannerSubscription scanSub;
    scanSub.instrument = "STOCK.EU";
    scanSub.locationCode = "STK.EU.IBIS";
    scanSub.scanCode = "TOP_PERC_GAIN";
    //! [toppercentgaineribis]
    return scanSub;
}

ScannerSubscription ScannerSubscriptionSamples::MostActiveFutEurex()
{
    //! [mostactivefuteurex]
    //Most active futures at EUREX
    ScannerSubscription scanSub;
    scanSub.instrument = "FUT.EU";
    scanSub.locationCode = "FUT.EU.EUREX";
    scanSub.scanCode = "MOST_ACTIVE";
    //! [mostactivefuteurex]
    return scanSub;
}

ScannerSubscription ScannerSubscriptionSamples::HighOptVolumePCRatioUSIndexes()
{
    //! [highoptvolume]
    //High option volume P/C ratio US indexes
    ScannerSubscription scanSub;
    scanSub.instrument = "IND.US";
    scanSub.locationCode = "IND.US";
    scanSub.scanCode = "HIGH_OPT_VOLUME_PUT_CALL_RATIO";
    //! [highoptvolume]
    return scanSub;
}

ScannerSubscription ScannerSubscriptionSamples::ComplexOrdersAndTrades()
{
    //! [combolatesttrade]
    //Complex orders and trades scan, latest trades
    ScannerSubscription scanSub;
    scanSub.instrument = "NATCOMB";
    scanSub.locationCode = "NATCOMB.OPT.US";
    scanSub.scanCode = "COMBO_LATEST_TRADE";
    //! [combolatesttrade]
    return scanSub;
}